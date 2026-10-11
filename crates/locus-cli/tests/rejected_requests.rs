//! Failure archives retain reproducible requests only in development builds.

use std::error::Error;
use std::fs;
use std::path::Path;

use assert_cmd::Command;
#[cfg(debug_assertions)]
use serde_json::Value;

fn locus(directory: &Path) -> Result<Command, Box<dyn Error>> {
    let mut command = Command::cargo_bin("locus")?;
    command.current_dir(directory);
    command.env("LOCUS_REJECTED_REQUESTS_DIR", directory.join("archive"));
    command.env_remove("LOCUS_REQUESTS_DIR");
    command.env("NO_COLOR", "1");
    Ok(command)
}

#[cfg(debug_assertions)]
fn records(directory: &Path) -> Result<Vec<Value>, Box<dyn Error>> {
    fs::read_dir(directory.join("archive"))?
        .map(|entry| Ok(serde_json::from_slice(&fs::read(entry?.path())?)?))
        .collect()
}

#[test]
fn archive_is_exclusive_to_debug_builds() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    fs::write(directory.path().join("bad.locus"), "point A = point()\n")?;
    locus(directory.path())?
        .args(["check", "bad.locus", "--json"])
        .assert()
        .code(3);
    assert_eq!(
        directory.path().join("archive").exists(),
        cfg!(debug_assertions)
    );
    Ok(())
}

#[test]
#[cfg(debug_assertions)]
fn syntax_failures_retain_original_source_and_diagnostics() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let source = "# original comment: 中文\npoint A = point()\n";
    fs::write(directory.path().join("bad.locus"), source)?;
    for json in [false, true] {
        let mut command = locus(directory.path())?;
        command.args(["check", "bad.locus"]);
        if json {
            command.arg("--json");
        }
        command.assert().code(3);
    }
    let saved = records(directory.path())?;
    assert_eq!(saved.len(), 2);
    for record in saved {
        assert_eq!(record["schema_version"], 1);
        assert_eq!(record["exit_code"], 3);
        assert_eq!(record["original_files"][0]["source"], source);
        assert_eq!(record["evaluated_sources"][0]["source"], source);
        assert_eq!(
            record["working_directory"],
            directory.path().canonicalize()?.to_string_lossy().as_ref()
        );
        assert!(
            record["error_names"]
                .as_array()
                .is_some_and(|names| { names.contains(&record["diagnostics"][0]["code"]) })
        );
        assert!(record["diagnostics"][0]["message"].is_string());
        assert!(record["diagnostics"][0]["span"].is_object());
        assert!(
            record["timestamp_unix_nanos"]
                .as_str()
                .is_some_and(|value| value.parse::<u128>().is_ok())
        );
    }
    Ok(())
}

#[test]
#[cfg(debug_assertions)]
fn rejected_edits_retain_stdin_arguments_and_candidate() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("scene.locus");
    let source = "locus 0 beta 5\npoint A = point()\nnum n = 1\n";
    let appended = "num bad = 1 / 0\n";
    fs::write(&path, source)?;
    locus(directory.path())?
        .args(["add", "scene.locus"])
        .write_stdin(appended)
        .assert()
        .code(2);
    let saved = records(directory.path())?;
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0]["stdin"], appended);
    assert_eq!(saved[0]["original_files"][0]["source"], source);
    assert_eq!(
        saved[0]["candidate_sources"][0]["source"],
        format!("{source}{appended}")
    );
    assert_eq!(saved[0]["error_names"][0], "E404");
    assert_eq!(fs::read_to_string(&path)?, source);
    fs::remove_dir_all(directory.path().join("archive"))?;

    locus(directory.path())?
        .args(["update", "scene.locus", "--id", "n", "--def", "1 / 0"])
        .assert()
        .code(2);
    let saved = records(directory.path())?;
    assert_eq!(saved[0]["original_files"][0]["source"], source);
    assert_eq!(
        saved[0]["candidate_sources"][0]["source"],
        "locus 0 beta 5\npoint A = point()\nnum n = 1 / 0\n"
    );
    assert!(
        saved[0]["arguments"]
            .as_array()
            .is_some_and(|args| args.iter().any(|arg| arg == "1 / 0"))
    );
    assert_eq!(fs::read_to_string(path)?, source);
    Ok(())
}

#[test]
#[cfg(debug_assertions)]
fn batch_assertions_and_strict_warnings_are_archived() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    fs::write(
        directory.path().join("assert.locus"),
        "locus 0 beta 5\nnum a = 1\nassert a == 2\n",
    )?;
    fs::write(directory.path().join("bad.locus"), "num a = 1\n")?;
    fs::write(
        directory.path().join("good.locus"),
        "locus 0 beta 5\nnum a = 1\nassert a == 1\n",
    )?;
    locus(directory.path())?
        .args(["check", "assert.locus", "bad.locus", "good.locus"])
        .assert()
        .code(3);
    let saved = records(directory.path())?;
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0]["original_files"].as_array().map(Vec::len), Some(3));
    assert_eq!(saved[0]["result"]["files"][2]["passed"], true);
    assert!(
        saved[0]["error_names"]
            .as_array()
            .is_some_and(|names| names.iter().any(|name| name == "assert_failed"))
    );
    assert_eq!(
        saved[0]["result"]["files"][0]["assertions"]["passed"],
        false
    );
    fs::remove_dir_all(directory.path().join("archive"))?;

    fs::write(
        directory.path().join("warning.locus"),
        "locus 0 beta 5\npoint A = point()\npoint B = offset(A, 0, 0)\n",
    )?;
    locus(directory.path())?
        .args(["check", "warning.locus", "--strict"])
        .assert()
        .code(1);
    let saved = records(directory.path())?;
    assert_eq!(saved[0]["exit_code"], 1);
    assert_eq!(saved[0]["diagnostics"][0]["severity"], "warning");
    assert_eq!(saved[0]["diagnostics"][0]["code"], "crowding");
    Ok(())
}

#[test]
#[cfg(debug_assertions)]
fn usage_io_and_non_utf8_failures_keep_available_context() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let source = "locus 0 beta 5\npoint A = point()\n";
    fs::write(directory.path().join("scene.locus"), source)?;
    locus(directory.path())?
        .args(["update", "scene.locus"])
        .assert()
        .code(3);
    let saved = records(directory.path())?;
    assert_eq!(saved[0]["error_names"][0], "usage");
    assert_eq!(saved[0]["original_files"][0]["source"], source);
    fs::remove_dir_all(directory.path().join("archive"))?;

    let bytes = vec![0xff, 0xfe, 0];
    fs::write(directory.path().join("binary.locus"), &bytes)?;
    locus(directory.path())?
        .args(["check", "missing.locus", "binary.locus"])
        .assert()
        .code(3);
    let saved = records(directory.path())?;
    assert_eq!(saved[0]["error_names"][0], "io");
    assert!(saved[0]["original_files"][0]["read_error"].is_string());
    assert_eq!(
        saved[0]["original_files"][1]["source_bytes"],
        serde_json::json!(bytes)
    );
    Ok(())
}

#[test]
#[cfg(debug_assertions)]
fn success_help_and_version_do_not_create_archives() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    fs::write(
        directory.path().join("good.locus"),
        "locus 0 beta 5\nnum a = 1\n",
    )?;
    locus(directory.path())?
        .args(["check", "good.locus"])
        .assert()
        .success();
    locus(directory.path())?.arg("--help").assert().success();
    locus(directory.path())?.arg("--version").assert().success();
    assert!(!directory.path().join("archive").exists());
    Ok(())
}

#[test]
#[cfg(debug_assertions)]
fn opt_in_capture_retains_success_failure_and_display_requests() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let source = "locus 0 beta 5\nnum a = 1\n";
    let appended = "num b = 2\n";
    fs::write(directory.path().join("good.locus"), source)?;
    locus(directory.path())?
        .env("LOCUS_REQUESTS_DIR", directory.path().join("requests"))
        .args(["add", "good.locus"])
        .write_stdin(appended)
        .assert()
        .success();
    let paths: Vec<_> =
        fs::read_dir(directory.path().join("requests"))?.collect::<Result<_, _>>()?;
    let saved: Value = serde_json::from_slice(&fs::read(paths[0].path())?)?;
    assert_eq!(saved["exit_code"], 0);
    assert_eq!(saved["stdin"], appended);
    assert_eq!(saved["original_files"][0]["source"], source);
    assert_eq!(
        saved["candidate_sources"][0]["source"],
        format!("{source}{appended}")
    );
    assert!(!directory.path().join("archive").exists());
    for arguments in [
        vec!["check", "missing.locus"],
        vec!["--help"],
        vec!["--version"],
    ] {
        let output = locus(directory.path())?
            .env("LOCUS_REQUESTS_DIR", directory.path().join("requests"))
            .args(arguments)
            .output()?;
        assert!(output.status.code().is_some());
    }
    assert_eq!(fs::read_dir(directory.path().join("requests"))?.count(), 4);
    assert_eq!(records(directory.path())?.len(), 1);
    Ok(())
}

#[test]
#[cfg(debug_assertions)]
fn archive_failure_preserves_json_and_exit_code() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    fs::write(directory.path().join("archive"), "blocked")?;
    let output = locus(directory.path())?
        .args(["check", "missing.locus", "--json"])
        .output()?;
    assert_eq!(output.status.code(), Some(3));
    let result: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(result["files"][0]["diagnostics"][0]["code"], "io");
    assert!(String::from_utf8(output.stderr)?.contains("warning: could not save rejected request"));
    Ok(())
}

#[test]
#[cfg(debug_assertions)]
fn default_archive_directory_is_relative_to_working_directory() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    locus(directory.path())?
        .env_remove("LOCUS_REJECTED_REQUESTS_DIR")
        .args(["check", "missing.locus"])
        .assert()
        .code(3);
    assert_eq!(
        fs::read_dir(directory.path().join(".locus/rejected-requests"))?.count(),
        1
    );
    Ok(())
}
