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

#[test]
fn onboard_writes_config_and_docs_handoff() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("rustez.json");
    let docs = dir.path().join("SETUP.md");
    let out = Command::new(bin())
        .args([
            "onboard",
            "--provider",
            "openai",
            "--token",
            "smoke-token",
            "--model",
            "test-model",
            "--non-interactive",
            "--skip-test",
            "--docs",
        ])
        .arg(&docs)
        .env("EZ_CONFIG_PATH", &cfg)
        .output()
        .expect("run onboard");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let cfg_text = std::fs::read_to_string(&cfg).unwrap();
    assert!(cfg_text.contains("test-model"), "{cfg_text}");
    assert!(
        !cfg_text.contains("smoke-token"),
        "token must not be inlined by default"
    );
    let docs_text = std::fs::read_to_string(&docs).unwrap();
    assert!(docs_text.contains("provider: openai"), "{docs_text}");
    assert!(docs_text.contains("test-model"), "{docs_text}");
    assert!(docs_text.contains("never print secrets"), "{docs_text}");
    assert!(docs_text.contains("- [ ]"), "resume checklist missing");
}

#[test]
fn onboard_rejects_unknown_provider() {
    let out = Command::new(bin())
        .args(["onboard", "--provider", "telegram"])
        .env("EZ_CONFIG_PATH", "/nonexistent-rustez-e2e-xyz.json")
        .output()
        .expect("run onboard bad provider");
    assert!(!out.status.success());
}
