//! Editing reusable beta 6 source preserves namespaces and dependency safety.

use std::error::Error;
use std::fs;

use assert_cmd::Command;

const INSTANCE: &str = "locus 0 beta 6\n\
fn duo(distance) {\n\
    point tail()\n\
    point tip = offset(tail, distance, 0)\n\
    segment path(tail, tip)\n\
    rel align_center_y(tail, tip)\n\
}\n\
duo sample(2) [highlight] // keep\n";

fn locus() -> Result<Command, Box<dyn Error>> {
    let mut command = Command::cargo_bin("locus")?;
    command.env("NO_COLOR", "1");
    Ok(command)
}

#[test]
fn instance_arguments_are_editable_and_unreferenced_instances_can_be_removed()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("instance.locus");
    fs::write(&path, INSTANCE)?;
    locus()?
        .arg("update")
        .arg(&path)
        .args(["--id", "sample", "--def", "duo(3)"])
        .assert()
        .success();
    let edited = fs::read_to_string(&path)?;
    assert!(edited.contains("duo sample(3) [highlight] // keep"));
    assert!(edited.contains("point tip = offset(tail, distance, 0)"));
    locus()?
        .arg("remove")
        .arg(&path)
        .args(["--id", "sample"])
        .assert()
        .success();
    let removed = fs::read_to_string(&path)?;
    assert!(removed.contains("fn duo(distance)"));
    assert!(!removed.contains("duo sample("));
    Ok(())
}

#[test]
fn qualified_members_identify_the_body_and_invocation_without_editing() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("member.locus");
    fs::write(&path, INSTANCE)?;
    for command in ["update", "remove"] {
        let mut request = locus()?;
        request
            .arg(command)
            .arg(&path)
            .args(["--id", "sample.tail", "--json"]);
        if command == "update" {
            request.args(["--def", "point()"]);
        }
        let output = request.output()?;
        assert_eq!(output.status.code(), Some(2));
        let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let diagnostic = &report["diagnostics"][0];
        assert_eq!(diagnostic["code"], "E305");
        let message = diagnostic["message"].as_str().ok_or("diagnostic message")?;
        assert!(message.contains("body line 3"), "{message}");
        assert!(message.contains("`duo` as `sample` at 8:1"), "{message}");
        assert_eq!(fs::read_to_string(&path)?, INSTANCE);
    }
    Ok(())
}

#[test]
fn instance_removal_rejects_external_declarations_and_assertions() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("dependents.locus");
    for suffix in [
        "point outside = sample.tail\n",
        "assert sample.path.length == 2\n",
        "group paths(sample)\n",
    ] {
        let source = format!("{INSTANCE}{suffix}");
        fs::write(&path, &source)?;
        let output = locus()?
            .arg("remove")
            .arg(&path)
            .args(["--id", "sample", "--json"])
            .output()?;
        assert_eq!(output.status.code(), Some(2));
        let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(report["diagnostics"][0]["code"], "E303");
        assert_eq!(fs::read_to_string(&path)?, source);
    }
    Ok(())
}

#[test]
fn expanded_call_dependencies_block_removing_an_argument_declaration() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("captured.locus");
    let source = "locus 0 beta 6\npoint anchor()\n\
        fn copy(original) {\npoint member = original\n}\ncopy copied(anchor)\n";
    fs::write(&path, source)?;
    let output = locus()?
        .arg("remove")
        .arg(&path)
        .args(["--id", "anchor", "--json"])
        .output()?;
    assert_eq!(output.status.code(), Some(2));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["diagnostics"][0]["code"], "E303");
    assert!(
        report["diagnostics"][0]["message"]
            .as_str()
            .ok_or("message")?
            .contains("copied.member")
    );
    assert_eq!(fs::read_to_string(&path)?, source);
    Ok(())
}

#[test]
fn groups_can_be_updated_and_block_member_removal() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("groups.locus");
    let source = "locus 0 beta 6\npoint A()\npoint B()\ngroup guides(A) // keep\n";
    fs::write(&path, source)?;
    locus()?
        .arg("update")
        .arg(&path)
        .args(["--id", "guides", "--def", "group(B)"])
        .assert()
        .success();
    assert!(fs::read_to_string(&path)?.contains("group guides(B) // keep"));
    locus()?
        .arg("remove")
        .arg(&path)
        .args(["--id", "A"])
        .assert()
        .success();
    let output = locus()?
        .arg("remove")
        .arg(&path)
        .args(["--id", "B", "--json"])
        .output()?;
    assert_eq!(output.status.code(), Some(2));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["diagnostics"][0]["code"], "E303");
    locus()?
        .arg("remove")
        .arg(&path)
        .args(["--id", "guides"])
        .assert()
        .success();
    Ok(())
}

#[test]
fn instance_updates_reject_fragments_that_escape_the_call() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("fragment.locus");
    for fragment in [
        "duo(3) // replaced",
        "duo(3)\nnum injected = 7",
        "duo(3) [main]",
    ] {
        fs::write(&path, INSTANCE)?;
        locus()?
            .arg("update")
            .arg(&path)
            .args(["--id", "sample", "--def", fragment])
            .assert()
            .code(3);
        assert_eq!(fs::read_to_string(&path)?, INSTANCE);
    }
    Ok(())
}

#[test]
fn constructor_definition_updates_preserve_id_and_suffix() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("constructor.locus");
    let source = "locus 0 beta 6\ncircle a(2) [highlight] // keep\ncircle b = a\n";
    fs::write(&path, source)?;
    for (id, definition) in [("a", "circle(3)"), ("b", "circle(4)")] {
        locus()?
            .arg("update")
            .arg(&path)
            .args(["--id", id, "--def", definition])
            .assert()
            .success();
    }
    let edited = fs::read_to_string(&path)?;
    assert!(edited.contains("circle a(3) [highlight] // keep"));
    assert!(edited.contains("circle b(4)"));
    locus()?
        .arg("update")
        .arg(&path)
        .args(["--id", "b", "--def", "a"])
        .assert()
        .success();
    assert!(fs::read_to_string(&path)?.contains("circle b = a"));
    Ok(())
}

#[test]
fn collection_relations_preserve_removal_dependencies_without_blocking_unrelated_edits()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("related-group.locus");
    let source = "locus 0 beta 6\npoint A()\npoint B = offset(A,2,0)\npoint unused() {mark:none}\ngroup pair(A,B)\nrel align_center_y(pair,A)\nassert center(pair,pair)\n";
    fs::write(&path, source)?;
    locus()?
        .arg("remove")
        .arg(&path)
        .args(["--id", "unused"])
        .assert()
        .success();
    let edited = fs::read_to_string(&path)?;
    assert!(!edited.contains("point unused"));
    for id in ["pair", "A"] {
        let output = locus()?
            .arg("remove")
            .arg(&path)
            .args(["--id", id, "--json"])
            .output()?;
        assert_eq!(output.status.code(), Some(2));
        let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(report["diagnostics"][0]["code"], "E303");
        assert!(
            report["diagnostics"][0]["message"]
                .as_str()
                .ok_or("message")?
                .contains("orphan dependents")
        );
        assert_eq!(fs::read_to_string(&path)?, edited);
    }
    Ok(())
}
