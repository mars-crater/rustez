//! `rustez-email`: email connector interface (impl via onboarding).

use serde::{Deserialize, Serialize};

/// Outbound email.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzEmailMsg {
    pub to: String,
    pub subject: String,
    pub body: String,
}

/// Email id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzEmailId(pub String);

/// Search query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzEmailQuery {
    pub text: String,
}

/// Search hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzEmailHit {
    pub id: EzEmailId,
    pub subject: String,
}

/// Email trait — v1: single SMTP/IMAP impl (wired in onboarding).
#[allow(async_fn_in_trait)]
pub trait EzEmail: Send + Sync {
    /// Kind, e.g. `"smtp"`.
    fn kind(&self) -> &'static str;
    /// Send (allowlisted + approval-gated).
    async fn send(&self, msg: EzEmailMsg) -> anyhow::Result<EzEmailId>;
    /// Search (for agent recall).
    async fn search(&self, query: EzEmailQuery) -> anyhow::Result<Vec<EzEmailHit>>;
}
