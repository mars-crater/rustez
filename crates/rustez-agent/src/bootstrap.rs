//! Bootstrap: welcome banner, OpenAI-first onboard flow helpers,
//! `docs/SETUP.md` handoff writer, and the agent resume pointer.
//! Heavy provider impls stay in onboarding; this module only carries the
//! first-run path (lightweight by design).

use rustez_config::{EzSecretInput, RustEzConfig, RustEzModelDef, RustEzProviderCfg};

/// First-run welcome banner printed by `rustez onboard` and hinted by `gateway`.
pub const WELCOME: &str = "\
RustEZ 0.1.0 — lightweight agent gateway
No config found. Let's set up OpenAI first (2 min), then an agent resumes the rest.
";

/// Default OpenAI model offered by the wizard.
pub const DEFAULT_OPENAI_MODEL: &str = "gpt-5.6-sol";

/// Default usage-poll interval (seconds) for the OpenAI subscription watcher.
pub const DEFAULT_POLL_SECS: u64 = 300;

/// Resume order after the OpenAI bootstrap step.
pub const RESUME_ORDER: &[&str] = &[
    "Discord channel (`just wiz discord`)",
    "Usage poll verify (`GET /api/usage/sub?provider=openai`)",
    "Memory Qdrant (`just wiz qdrant`)",
    "Secrets Proton Pass (`just wiz proton-pass`)",
    "Email (`just wiz email`)",
];

/// Agent created with the bootstrapped provider+model, pointed at the setup doc
/// so it can resume the rest of the setup.
#[derive(Debug, Clone)]
pub struct EzAgentBootstrap {
    /// e.g. `"openai"`.
    pub provider: String,
    /// e.g. `"gpt-5.6-sol"`.
    pub model: String,
    /// Handoff doc, e.g. `"docs/SETUP.md"`.
    pub docs: String,
}

impl EzAgentBootstrap {
    /// System-prompt prefix for the setup agent.
    pub fn prompt(&self) -> String {
        format!(
            "You are the RustEZ setup agent ({}/{}). Read {} and the config it points to, \
            then continue with the next unchecked wizard item and update the checklist. \
            Keep changes minimal and never print secrets.",
            self.provider, self.model, self.docs
        )
    }
}

/// Apply the OpenAI bootstrap answer to a config (token stored as given —
/// caller decides env-ref vs literal).
pub fn apply_openai(cfg: &mut RustEzConfig, secret: EzSecretInput, model: &str) {
    cfg.providers.openai = Some(RustEzProviderCfg {
        base_url: String::new(),
        api_key: Some(secret),
        api: "openai-completions".to_string(),
        timeout_seconds: Some(30),
        models: vec![RustEzModelDef {
            id: model.to_string(),
            name: None,
        }],
    });
}

/// Write the `docs/SETUP.md` handoff doc (front-matter + checklist).
pub fn write_setup_doc(
    path: &str,
    provider: &str,
    model: &str,
    config_path: &str,
    ping_ok: bool,
) -> anyhow::Result<()> {
    if let Some(parent) = std::path::Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let ping_line = if ping_ok {
        "- [x] OpenAI provider + model (test ping ok)"
    } else {
        "- [ ] OpenAI provider + model (ping skipped — rerun `rustez onboard`)"
    };
    let mut rest = String::new();
    for item in RESUME_ORDER {
        rest.push_str(&format!("- [ ] {item}\n"));
    }
    let prompt = EzAgentBootstrap {
        provider: provider.to_string(),
        model: model.to_string(),
        docs: path.to_string(),
    }
    .prompt();
    let body = format!(
        "---\nprovider: {provider}\nmodel: {model}\nconfig: {config_path}\nstatus: bootstrapped\n---\n\
        # RustEZ setup — resume from here\n\n\
        New here? Read docs/AGENT.md first — the shared manual for any agent working with RustEZ.\n\n\
        {ping_line}\n{rest}\n\
        ## Agent\n\n\
        {prompt}\n"
    );
    std::fs::write(path, body)?;
    Ok(())
}

/// Parse the checklist in a setup doc: returns `(done, total)` or `None` if unreadable.
pub fn setup_progress(path: &str) -> Option<(usize, usize)> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut done = 0;
    let mut total = 0;
    for line in text.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("- [x]") {
            let _ = rest;
            done += 1;
            total += 1;
        } else if t.strip_prefix("- [ ]").is_some() {
            total += 1;
        }
    }
    if total == 0 {
        None
    } else {
        Some((done, total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_points_at_docs() {
        let b = EzAgentBootstrap {
            provider: "openai".to_string(),
            model: "gpt-5.6-sol".to_string(),
            docs: "docs/SETUP.md".to_string(),
        };
        let p = b.prompt();
        assert!(p.contains("openai/gpt-5.6-sol"));
        assert!(p.contains("docs/SETUP.md"));
        assert!(p.contains("never print secrets"));
    }

    #[test]
    fn setup_doc_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SETUP.md");
        let ps = path.to_str().unwrap();
        write_setup_doc(ps, "openai", "gpt-5.6-sol", "./rustez.json", true).unwrap();
        let (done, total) = setup_progress(ps).unwrap();
        assert_eq!(done, 1);
        assert_eq!(total, 1 + RESUME_ORDER.len());
        let text = std::fs::read_to_string(ps).unwrap();
        assert!(text.contains("provider: openai"));
        assert!(text.contains("never print secrets"));
    }

    #[test]
    fn apply_openai_sets_provider() {
        let mut cfg = RustEzConfig::default();
        apply_openai(
            &mut cfg,
            EzSecretInput::Literal("t".to_string()),
            "gpt-5.6-sol",
        );
        let p = cfg.providers.openai.unwrap();
        assert_eq!(p.models[0].id, "gpt-5.6-sol");
    }
}
