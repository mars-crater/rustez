//! `rustez-voice`: TTS + STT traits (impls via onboarding).

use serde::{Deserialize, Serialize};

/// TTS request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzTtsReq {
    pub text: String,
    #[serde(default)]
    pub voice: String,
}

/// Audio blob (opaque in skeleton).
#[derive(Debug, Clone)]
pub struct EzAudio {
    pub bytes: Vec<u8>,
    pub format: String,
}

/// Audio input for STT.
#[derive(Debug, Clone)]
pub struct EzAudioIn {
    pub bytes: Vec<u8>,
}

/// TTS trait.
#[allow(async_fn_in_trait)]
pub trait EzTts: Send + Sync {
    /// Kind, e.g. `"local-cli"`.
    fn kind(&self) -> &'static str;
    /// Speak.
    async fn speak(&self, req: EzTtsReq) -> anyhow::Result<EzAudio>;
}

/// STT trait.
#[allow(async_fn_in_trait)]
pub trait EzStt: Send + Sync {
    /// Kind, e.g. `"whisper-file"`.
    fn kind(&self) -> &'static str;
    /// Transcribe.
    async fn transcribe(&self, audio: EzAudioIn) -> anyhow::Result<String>;
}
