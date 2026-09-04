//! `rustez-compat`: openclaw → rustez key/env/wire shims only.
//! Only crate allowed to mention `claw`/`openclaw` outside tests.

use serde_json::Value;

/// Map a legacy `openclaw.json` key to its `rustez.json` equivalent (stub).
pub fn from_openclaw_key(key: &str) -> &str {
    match key {
        "openclaw.json" => "rustez.json",
        _ => key,
    }
}

/// Read `EZ_*` with fallback to legacy `OPENCLAW_*` (stub — real impl on onboarding).
pub fn ez_env(name: &str) -> Option<String> {
    std::env::var(format!("EZ_{name}"))
        .or_else(|_| std::env::var(format!("RUSTEZ_{name}")))
        .or_else(|_| std::env::var(format!("OPENCLAW_{name}")))
        .ok()
}

/// Passthrough JSON value (keeps unknown keys — compat rule).
pub fn passthrough(v: Value) -> Value {
    v
}
