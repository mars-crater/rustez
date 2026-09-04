//! `rustez-agent`: provider + wizard traits (no impls — onboarding owns them).

use serde::{Deserialize, Serialize};

/// Bootstrap: welcome banner, OpenAI-first onboard helpers, docs handoff.
pub mod bootstrap;

/// Chat request (minimal).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzChatReq {
    pub provider: String,
    pub model: String,
    pub input: String,
}

/// Chat response (minimal).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzChatResp {
    pub text: String,
}

/// Provider trait (v1 ids: `openai`, `opencode-go`, `chutes`).
#[allow(async_fn_in_trait)]
pub trait EzProvider: Send + Sync {
    /// Provider id.
    fn id(&self) -> &'static str;
    /// Chat (impl via onboarding wizard).
    async fn chat(&self, req: EzChatReq) -> anyhow::Result<EzChatResp>;
    /// Subscription usage % (v1 focus; token spendings deferred to TODO).
    async fn sub_usage(&self) -> anyhow::Result<rustez_usage::EzSubUsage>;
}

/// Wizard definition served to UI (`GET /api/wizards/:impl`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzWizardStep {
    pub id: String,
    pub title: String,
    pub help: String,
}
