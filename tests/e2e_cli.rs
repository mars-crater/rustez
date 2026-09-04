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
fn onboard_print_url_needs_no_network() {
    let out = Command::new(bin())
        .args(["onboard", "--provider", "openai", "--print-url"])
        .env("EZ_CONFIG_PATH", "/nonexistent-rustez-e2e-xyz.json")
        .output()
        .expect("run onboard --print-url");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be JSON handoff");
    let url = v["url"].as_str().unwrap_or("");
    assert!(
        url.contains("https://auth.openai.com/oauth/authorize"),
        "{url}"
    );
    assert!(url.contains("code_challenge"), "{url}");
    assert!(!v["verifier"].as_str().unwrap_or("").is_empty());
    assert!(!v["state"].as_str().unwrap_or("").is_empty());
}

#[test]
fn onboard_non_interactive_needs_paste_handoff() {
    let out = Command::new(bin())
        .args(["onboard", "--provider", "openai", "--non-interactive"])
        .env("EZ_CONFIG_PATH", "/nonexistent-rustez-e2e-xyz.json")
        .output()
        .expect("run onboard non-interactive");
    assert!(!out.status.success(), "dance needs a human or --paste-code");
}

#[test]
fn auth_status_clean_when_never_logged_in() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(bin())
        .args(["auth", "status", "--provider", "openai"])
        .env("RUSTEZ_AUTH_DIR", dir.path())
        .output()
        .expect("run auth status");
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("not logged in"), "{text}");
    assert!(!text.contains("eyJ"), "must never leak token material");
}

#[test]
fn auth_login_rejects_unknown_provider() {
    let out = Command::new(bin())
        .args(["auth", "login", "--provider", "telegram"])
        .env("RUSTEZ_AUTH_DIR", "/nonexistent-rustez-e2e-xyz")
        .output()
        .expect("run auth login bad provider");
    assert!(!out.status.success());
}
