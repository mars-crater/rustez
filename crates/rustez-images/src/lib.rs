//! `rustez-images`: image creation trait (impl via onboarding).

use serde::{Deserialize, Serialize};

/// Image prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzImagePrompt {
    pub text: String,
    #[serde(default)]
    pub size: String,
}

/// Generated image (file-out in v1 impl).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzImage {
    pub path: String,
}

/// Image trait.
#[allow(async_fn_in_trait)]
pub trait EzImageGen: Send + Sync {
    /// Kind, e.g. `"file-out"`.
    fn kind(&self) -> &'static str;
    /// Generate.
    async fn generate(&self, prompt: EzImagePrompt) -> anyhow::Result<EzImage>;
}
