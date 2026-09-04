//! `rustez-channels`: global channel interface. Discord-only in v1.

use serde::{Deserialize, Serialize};

/// Outbound message (minimal).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzOutMsg {
    pub peer: String,
    pub text: String,
}

/// Channel context handed to impls.
#[derive(Debug, Clone)]
pub struct EzChanCtx {
    pub gateway_port: u16,
}

/// Global channel trait — all channels (discord now, rest in TODO) implement this.
#[allow(async_fn_in_trait)]
pub trait EzChannel: Send + Sync {
    /// Channel id, e.g. `"discord"`.
    fn id(&self) -> &'static str;
    /// Start listener (long-poll / webhook — onboarding wizard configures).
    async fn start(&self, ctx: EzChanCtx) -> anyhow::Result<()>;
    /// Send a message.
    async fn send(&self, msg: EzOutMsg) -> anyhow::Result<()>;
}
