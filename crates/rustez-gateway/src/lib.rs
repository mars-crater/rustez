//! `rustez-gateway`: minimal axum serve (health + config + wizard stubs).
//! WS/RPC, pairing, and onboarding impls stay in TODO — keeps binary light.

use axum::{routing::get, Json, Router};
use serde::{Deserialize, Serialize};

/// Default test port (Node OpenClaw keeps 18789).
pub const RUSTEZ_DEFAULT_PORT: u16 = 18790;

/// Gateway handle.
#[derive(Debug, Clone)]
pub struct EzGateway {
    pub port: u16,
}

impl EzGateway {
    /// Create a handle (bind happens in `serve`).
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    /// Serve `GET /healthz`, `GET /api/config`, `GET /api/wizards/:impl` stubs.
    pub async fn serve(&self) -> anyhow::Result<()> {
        let app = router();
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

/// Router (extracted for tests).
pub fn router() -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/config", get(get_config))
        .route("/api/wizards/:key", get(get_wizard))
}

async fn healthz() -> Json<EzHealth> {
    Json(EzHealth {
        ok: true,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

async fn get_config() -> Json<rustez_config::RustEzConfig> {
    let path = rustez_config::rustez_path();
    let cfg = rustez_config::rustez_load(&path).unwrap_or_default();
    // Never leak secret literals: mask `Literal` values on read.
    Json(mask_secrets(cfg))
}

async fn get_wizard(axum::extract::Path(key): axum::extract::Path<String>) -> Json<EzWizard> {
    Json(EzWizard::stub(&key))
}

/// Mask secret literals before serving config to UI.
fn mask_secrets(mut cfg: rustez_config::RustEzConfig) -> rustez_config::RustEzConfig {
    for p in [
        &mut cfg.providers.openai,
        &mut cfg.providers.opencode_go,
        &mut cfg.providers.chutes,
    ]
    .into_iter()
    .flatten()
    {
        if let Some(k) = &mut p.api_key {
            if matches!(k, rustez_config::EzSecretInput::Literal(_)) {
                *k = rustez_config::EzSecretInput::Literal("***".to_string());
            }
        }
    }
    if let Some(d) = &mut cfg.channels.discord {
        if let Some(t) = &mut d.token {
            if matches!(t, rustez_config::EzSecretInput::Literal(_)) {
                *t = rustez_config::EzSecretInput::Literal("***".to_string());
            }
        }
    }
    cfg
}

/// Wizard served to focused-node UI — auth + config fields mirror the interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzWizard {
    pub key: String,
    pub title: String,
    pub steps: Vec<EzWizardStep>,
    pub supported: bool,
}

/// Single wizard step with fields (no hardcoded UI — rendered from schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzWizardStep {
    pub id: String,
    pub title: String,
    pub help: String,
    #[serde(default)]
    pub fields: Vec<EzField>,
}

/// One auth or config field. `auth=true` marks credentials (always secret-handled).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzField {
    pub key: String,
    pub label: String,
    /// `text|secret|url|select|bool|number`.
    pub kind: String,
    pub required: bool,
    pub auth: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_hint: Option<String>,
}

impl EzField {
    fn auth(key: &str, label: &str, env_hint: &str) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            kind: "secret".to_string(),
            required: true,
            auth: true,
            placeholder: None,
            env_hint: Some(env_hint.to_string()),
        }
    }

    fn cfg(key: &str, label: &str, kind: &str, required: bool) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            kind: kind.to_string(),
            required,
            auth: false,
            placeholder: None,
            env_hint: None,
        }
    }
}

fn auth_step(id: &str, help: &str, fields: Vec<EzField>) -> EzWizardStep {
    EzWizardStep {
        id: id.to_string(),
        title: "Auth".to_string(),
        help: help.to_string(),
        fields,
    }
}

fn config_step(help: &str, fields: Vec<EzField>) -> EzWizardStep {
    EzWizardStep {
        id: "config".to_string(),
        title: "Config".to_string(),
        help: help.to_string(),
        fields,
    }
}

impl EzWizard {
    /// Static schema per impl key — mirrors the trimmed `rustez-config` shapes.
    pub fn stub(key: &str) -> Self {
        let steps = match key {
            "discord" => vec![
                auth_step(
                    "auth",
                    "Bot token — prefer env over pasting.",
                    vec![EzField::auth("token", "Bot token", "EZ_DISCORD_TOKEN")],
                ),
                config_step(
                    "Channel policy — maps to channels.discord{dmPolicy,allowFrom,requireMention,historyLimit}.",
                    vec![
                        EzField::cfg("dmPolicy", "DM policy", "select", false),
                        EzField::cfg("allowFrom", "Allow from (ids)", "text", false),
                        EzField::cfg("requireMention", "Require mention", "bool", false),
                        EzField::cfg("historyLimit", "History limit", "number", false),
                    ],
                ),
            ],
            "openai" => vec![
                auth_step(
                    "auth",
                    "Subscription auth — OAuth/setup token, never a raw API key in v1.",
                    vec![EzField::auth(
                        "subToken",
                        "Subscription token",
                        "EZ_OPENAI_SUB_TOKEN",
                    )],
                ),
                config_step(
                    "Model + usage poll — maps to providers.openai{models,usage}.",
                    vec![
                        EzField::cfg("model", "Primary model", "text", false),
                        EzField::cfg("pollSecs", "Usage poll seconds", "number", false),
                    ],
                ),
            ],
            "opencode-go" => vec![
                auth_step(
                    "auth",
                    "API token for opencode-go.",
                    vec![EzField::auth(
                        "apiToken",
                        "API token",
                        "EZ_OPENCODEGO_TOKEN",
                    )],
                ),
                config_step(
                    "Endpoint + models — maps to providers.opencode-go{baseUrl,models}.",
                    vec![
                        EzField::cfg("baseUrl", "Base URL", "url", false),
                        EzField::cfg("models", "Models (comma sep)", "text", false),
                    ],
                ),
            ],
            "chutes" => vec![
                auth_step(
                    "auth",
                    "API token for chutes.",
                    vec![EzField::auth("apiToken", "API token", "EZ_CHUTES_TOKEN")],
                ),
                config_step(
                    "Endpoint + models — maps to providers.chutes{baseUrl,models}.",
                    vec![
                        EzField::cfg("baseUrl", "Base URL", "url", false),
                        EzField::cfg("models", "Models (comma sep)", "text", false),
                    ],
                ),
            ],
            "qdrant" => vec![
                auth_step(
                    "auth",
                    "Optional API key — local Qdrant usually needs none.",
                    vec![EzField {
                        key: "apiKey".to_string(),
                        label: "API key (optional)".to_string(),
                        kind: "secret".to_string(),
                        required: false,
                        auth: true,
                        placeholder: None,
                        env_hint: Some("RUSTEZ_QDRANT_KEY".to_string()),
                    }],
                ),
                config_step(
                    "Recall index — maps to memory{provider:qdrant,remote,model}.",
                    vec![
                        EzField::cfg("url", "Qdrant URL", "url", true),
                        EzField::cfg("collection", "Collection", "text", true),
                        EzField::cfg("embedModel", "Embed model", "text", false),
                    ],
                ),
            ],
            "proton-pass" => vec![
                auth_step(
                    "auth",
                    "Automizer token — resolve-only probe, value never echoed.",
                    vec![EzField::auth(
                        "automizerToken",
                        "Automizer token",
                        "EZ_PROTONPASS_AUTOMIZER_TOKEN",
                    )],
                ),
                config_step(
                    "Store binding — maps to secrets{store:proton-pass}.",
                    vec![EzField::cfg("storeId", "Store ID", "text", false)],
                ),
            ],
            "email" => vec![
                auth_step(
                    "auth",
                    "SMTP credentials — allowlisted sending only.",
                    vec![
                        EzField::auth("smtpUser", "SMTP user", "EZ_SMTP_USER"),
                        EzField::auth("smtpPass", "SMTP pass", "EZ_SMTP_PASS"),
                    ],
                ),
                config_step(
                    "Endpoint + policy — maps to email{host,allowlist}.",
                    vec![
                        EzField::cfg("host", "SMTP host", "text", true),
                        EzField::cfg("port", "SMTP port", "number", false),
                        EzField::cfg("allowTo", "Allow to (comma sep)", "text", false),
                    ],
                ),
            ],
            _ => vec![EzWizardStep {
                id: "basics".to_string(),
                title: "Basics".to_string(),
                help: "Not in v1 — see TODO.md.".to_string(),
                fields: vec![],
            }],
        };
        let supported = matches!(
            key,
            "discord" | "openai" | "opencode-go" | "chutes" | "qdrant" | "proton-pass" | "email"
        );
        Self {
            key: key.to_string(),
            title: format!("{key} setup"),
            steps,
            supported,
        }
    }
}

/// Health payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzHealth {
    pub ok: bool,
    pub version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wizards_carry_auth_and_config() {
        for key in [
            "discord",
            "openai",
            "opencode-go",
            "chutes",
            "qdrant",
            "proton-pass",
            "email",
        ] {
            let w = EzWizard::stub(key);
            assert!(w.supported, "{key}");
            assert_eq!(w.steps.len(), 2, "{key}");
            assert!(
                w.steps[0].fields.iter().any(|f| f.auth),
                "{key} needs auth field"
            );
            assert!(
                w.steps[1].fields.iter().any(|f| !f.auth),
                "{key} needs config field"
            );
        }
        assert!(!EzWizard::stub("telegram").supported);
    }
}
