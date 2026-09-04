//! `rustez-memory`: memory interface, Qdrant-minded (impl via onboarding).

use serde::{Deserialize, Serialize};

/// Memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzMemEntry {
    pub text: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Memory id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzMemId(pub String);

/// Search query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzMemQuery {
    pub text: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    8
}

/// Search hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzMemHit {
    pub id: EzMemId,
    pub text: String,
    pub score: f32,
}

/// Global memory trait — v1 impl is Qdrant (wired in onboarding wizard).
#[allow(async_fn_in_trait)]
pub trait EzMemory: Send + Sync {
    /// Kind, e.g. `"qdrant"`.
    fn kind(&self) -> &'static str;
    /// Store.
    async fn put(&self, entry: EzMemEntry) -> anyhow::Result<EzMemId>;
    /// Recall.
    async fn search(&self, query: EzMemQuery) -> anyhow::Result<Vec<EzMemHit>>;
    /// Fetch by id.
    async fn get(&self, id: EzMemId) -> anyhow::Result<EzMemEntry>;
}
