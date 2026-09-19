//! Opt-in real `DeepSeek` Flash request followed by validation and native rendering.

use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use locus_core::{AssertionReport, ResolvedScene, evaluate_assertions, solve};
use locus_layout::{LayoutOptions, boxes::measure_scene, layout};
use locus_render::{
    RenderOptions, render_svg,
    style::{Color, StyleSheet},
};
use locus_tex::{FontSize, PlaintextEngine, RexEngine};
use serde_json::{Value, json};

const PROMPT: &str = include_str!("../../../spec/test/llm/prompt.txt");
const SYSTEM_PROMPT: &str = include_str!("../../../spec/test/llm/system-prompt.md");
// These checks belong to the test, not to the model's answer. They are evaluated
// against the original solved scene and never written into generated.locus.
const CONTRACT: &str = r"
assert horizontal(A, B) and vertical(A, C)
assert dist(A, B) == 4 and dist(A, C) == 3 and dist(B, C) == 5
assert area(tri) == 6
assert coincident(H, foot(A, line(B, C))) and between(B, H, C)
assert length(altitude) == 2.4 and on(A, altitude) and on(H, altitude)
assert perpendicular(altitude, seg(B, C))
assert on(A, circum) and on(B, circum) and on(C, circum)
assert radius(circum) == 2.5
assert radius(inc) == 1 and inside(center(inc), tri)
assert tangent(inc, seg(A, B))
assert tangent(inc, seg(A, C))
assert tangent(inc, seg(B, C))
";

fn output_directory() -> Result<PathBuf, Box<dyn Error>> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/llm-one-shot");
    // Each run keeps its own evidence, including failed or truncated responses.
    let run = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let directory = directory.join(format!("{run}-{}", std::process::id()));
    fs::create_dir_all(&directory)?;
    Ok(directory.canonicalize()?)
}

fn request_once(directory: &Path, key: &str) -> Result<Value, Box<dyn Error>> {
    let request = json!({
        "model": "deepseek-flash",
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": PROMPT}
        ],
        "thinking": {"type": "disabled"},
        "max_tokens": 8192,
        "stream": false
    });
    fs::write(directory.join("system-prompt.txt"), SYSTEM_PROMPT)?;
    fs::write(directory.join("prompt.txt"), PROMPT)?;
    let request_path = directory.join("request.json");
    fs::write(&request_path, serde_json::to_vec_pretty(&request)?)?;
    // Pass authentication through stdin, so the key is absent from argv and artifacts.
    // -q disables user curl configuration, which could otherwise enable retries.
    let mut child = Command::new("curl")
        .args([
            "-q",
            "--silent",
            "--show-error",
            "--fail-with-body",
            "--connect-timeout",
            "20",
            "--max-time",
            "180",
            "--request",
            "POST",
            "--url",
            "https://api.deepseek.com/chat/completions",
            "--header",
            "Content-Type: application/json",
            "--config",
            "-",
            "--data-binary",
        ])
        .arg(format!("@{}", request_path.display()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut input = child.stdin.take().ok_or("curl stdin unavailable")?;
    writeln!(input, "header = \"Authorization: Bearer {key}\"")?;
    drop(input);
    let output = child.wait_with_output()?;
    fs::write(directory.join("response.json"), &output.stdout)?;
    if !output.status.success() {
        return Err(format!(
            "DeepSeek request failed ({}): {}; see response.json in {}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
            directory.display()
        )
        .into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn check_contract(source: &str, scene: &ResolvedScene) -> Result<AssertionReport, Box<dyn Error>> {
    let parsed = locus_syntax::parse(&format!("{source}\n{CONTRACT}"));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    Ok(evaluate_assertions(&parsed.file, scene)?)
}

fn render_answer(source: &str, directory: &Path) -> Result<(), Box<dyn Error>> {
    let parsed = locus_syntax::parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.file.version, 3);
    let solved = solve(&parsed.file)?;
    let assertions = evaluate_assertions(&parsed.file, &solved.scene)?;
    fs::write(
        directory.join("assertions.json"),
        serde_json::to_vec_pretty(&assertions)?,
    )?;
    assert!(assertions.passed, "model assertions failed: {assertions:?}");
    assert!(
        assertions.assertions.len() >= 6,
        "expected at least six model assertions"
    );
    let contract = check_contract(source, &solved.scene)?;
    fs::write(
        directory.join("contract.json"),
        serde_json::to_vec_pretty(&contract)?,
    )?;
    assert!(contract.passed, "requested geometry failed: {contract:?}");

    let measured = measure_scene(
        &solved.scene,
        &PlaintextEngine::new(),
        &RexEngine,
        FontSize::DEFAULT,
    )?;
    let placed = layout(
        &solved.scene,
        &measured,
        LayoutOptions {
            em_size: 0.28,
            ..LayoutOptions::default()
        },
    )?;
    let styles = StyleSheet::from_file(&parsed.file)?;
    let options = RenderOptions {
        background: Some(Color(255, 255, 255)),
        ..RenderOptions::default()
    };
    let svg = render_svg(&solved.scene, &placed, &styles, options)?;
    let document = roxmltree::Document::parse(&svg)?;
    assert_eq!(document.root_element().tag_name().name(), "svg");
    fs::write(directory.join("generated.svg"), svg)?;
    #[cfg(feature = "png")]
    {
        let png = locus_render::png::render(&solved.scene, &placed, &styles, options)?;
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        fs::write(directory.join("generated.png"), png)?;
    }
    Ok(())
}

#[test]
fn independent_contract_rejects_a_valid_but_wrong_incircle() -> Result<(), Box<dyn Error>> {
    // Offline validation of the test's oracle; this is never used as an API answer.
    let source = format!(
        "{}\npoint H = foot(A, line(B, C))\nsegment altitude = seg(A, H)\n\
         circle inc = incircle(A, B, C)\n",
        include_str!("../../../spec/test/demonstrations/point_relations.locus")
    );
    let parsed = locus_syntax::parse(&source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let solved = solve(&parsed.file)?;
    assert!(check_contract(&source, &solved.scene)?.passed);

    // A unit circle at the circumcenter is valid Locus and has the requested
    // radius, but is not the incircle. The model's original assertions still pass.
    let wrong = source.replace("incircle(A, B, C)", "circle(M, 1)");
    let parsed = locus_syntax::parse(&wrong);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let solved = solve(&parsed.file)?;
    assert!(evaluate_assertions(&parsed.file, &solved.scene)?.passed);
    assert!(!check_contract(&wrong, &solved.scene)?.passed);
    Ok(())
}

#[test]
#[ignore = "calls the real DeepSeek API; requires DEEPSEEK_API_KEY and curl"]
fn deepseek_flash_one_shot_draws_requested_geometry() -> Result<(), Box<dyn Error>> {
    let key = std::env::var("DEEPSEEK_API_KEY")
        .map_err(|_| "set DEEPSEEK_API_KEY (see spec/test/llm/README.md)")?;
    let key = key.trim();
    if key.is_empty()
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    {
        return Err("DEEPSEEK_API_KEY is empty or contains invalid characters".into());
    }
    let directory = output_directory()?;
    eprintln!("One-shot DeepSeek Flash artifacts: {}", directory.display());
    let response = request_once(&directory, key)?;
    let choice = &response["choices"][0];
    let mut source = choice["message"]["content"]
        .as_str()
        .ok_or("missing Locus response content")?
        .to_owned();
    // Message text often omits the file terminator. Preserve the raw response
    // in response.json; add only the newline required by the source-file format.
    if !source.ends_with('\n') {
        source.push('\n');
    }
    fs::write(directory.join("generated.locus"), &source)?;
    assert_eq!(
        choice["finish_reason"], "stop",
        "incomplete DeepSeek response"
    );
    render_answer(&source, &directory)
}
