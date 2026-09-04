//! `rustez-secrets`: BYO secret-manager interface. Proton Pass first (via onboarding).

/// Secret reference (never logs value).
#[derive(Debug, Clone)]
pub struct EzSecretRef {
    pub id: String,
}

/// Redacted secret handle.
#[derive(Debug)]
pub struct EzSecret {
    _private: (),
}

/// Secret-store trait — bring your own manager.
#[allow(async_fn_in_trait)]
pub trait EzSecretStore: Send + Sync {
    /// Kind, e.g. `"proton-pass"`.
    fn kind(&self) -> &'static str;
    /// Resolve by id (automizer-token auth configured in onboarding wizard).
    async fn resolve(&self, id: &str) -> anyhow::Result<String>;
}
