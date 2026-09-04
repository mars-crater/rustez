//! e2e: CLI — `status` / `doctor` behave on good, missing, and bad configs.

use std::process::Command;

fn bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_rustez"))
}

#[test]
fn status_reports_port_and_path() {
    let out = Command::new(bin())
        .arg("status")
        .env("EZ_CONFIG_PATH", "/nonexistent-rustez-e2e-xyz.json")
        .output()
        .expect("run status");
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("18790"), "default port missing: {text}");
}

#[test]
fn doctor_ok_empty_and_discord() {
    // Empty/missing -> ok with discord false.
    let out = Command::new(bin())
        .arg("doctor")
        .env("EZ_CONFIG_PATH", "/nonexistent-rustez-e2e-xyz.json")
        .output()
        .expect("run doctor");
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("doctor ok"), "{text}");
    assert!(text.contains("discord false"), "{text}");

    // Discord config -> discord true.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rustez.json");
    std::fs::write(
        &path,
        r#"{"channels":{"discord":{"enabled":true,"dmPolicy":"pairing"}}}"#,
    )
    .unwrap();
    let out = Command::new(bin())
        .arg("doctor")
        .env("EZ_CONFIG_PATH", &path)
        .output()
        .expect("run doctor discord");
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("discord true"), "{text}");
}

#[test]
fn doctor_fails_on_invalid_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rustez.json");
    std::fs::write(&path, "{not json").unwrap();
    let out = Command::new(bin())
        .arg("doctor")
        .env("EZ_CONFIG_PATH", &path)
        .output()
        .expect("run doctor bad json");
    assert!(!out.status.success(), "doctor must fail on invalid JSON");
}
