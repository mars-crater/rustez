//! `rustez-config`: trimmed OpenClaw-like schemas for tiny scope.
//! v1: gateway subset + discord only + 3 providers + memory/MCP subset.
//! Wire keys stay `camelCase` like upstream; code idents stay lint-clean.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root config — trimmed tiny scope, passthrough for unknown keys.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RustEzConfig {
    #[serde(default)]
    pub gateway: RustEzGatewayCfg,
    #[serde(default)]
    pub providers: RustEzProvidersCfg,
    #[serde(default)]
    pub channels: RustEzChannelsCfg,
    #[serde(default)]
    pub memory: RustEzMemoryCfg,
    #[serde(default)]
    pub mcp: RustEzMcpCfg,
    /// Preserve unknown keys (OpenClaw `.passthrough()` rule).
    #[serde(flatten, default)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `string | SecretRef` (OpenClaw `SecretInput`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EzSecretInput {
    /// Literal value (empty = missing).
    Literal(String),
    /// Reference `{source, provider, id}`.
    Ref(EzSecretRef),
}

/// Secret reference — exactly 3 keys like upstream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzSecretRef {
    /// `env|file|exec|store`.
    pub source: String,
    pub provider: String,
    pub id: String,
}

/// `string|number` allowlist entries (Discord `allowFrom`, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EzAllowFrom {
    /// Name/handle.
    Name(String),
    /// Numeric id.
    Id(i64),
}

/// Gateway subset (upstream `GatewayConfig` trimmed).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RustEzGatewayCfg {
    /// Upstream default 18789; we test on 18790 alongside Node.
    #[serde(default = "default_port")]
    pub port: u16,
    /// `local|remote` (default `local`).
    #[serde(default = "default_local")]
    pub mode: String,
    /// `auto|lan|loopback|custom|tailnet` (default `loopback`).
    #[serde(default = "default_loopback")]
    pub bind: String,
    #[serde(default)]
    pub auth: RustEzAuthCfg,
}

fn default_port() -> u16 {
    18790
}

fn default_local() -> String {
    "local".to_string()
}

fn default_loopback() -> String {
    "loopback".to_string()
}

impl Default for RustEzGatewayCfg {
    fn default() -> Self {
        Self {
            port: default_port(),
            mode: default_local(),
            bind: default_loopback(),
            auth: RustEzAuthCfg::default(),
        }
    }
}

/// Auth subset: `mode none|token|password` + token ref.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RustEzAuthCfg {
    #[serde(default = "default_token_mode")]
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<EzSecretInput>,
}

fn default_token_mode() -> String {
    "token".to_string()
}

impl Default for RustEzAuthCfg {
    fn default() -> Self {
        Self {
            mode: default_token_mode(),
            token: None,
        }
    }
}

/// Providers v1: openai (sub) + opencode-go/chutes (tokens).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RustEzProvidersCfg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai: Option<RustEzProviderCfg>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opencode_go: Option<RustEzProviderCfg>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chutes: Option<RustEzProviderCfg>,
}

/// Single provider (upstream `ModelProviderConfig` trimmed).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RustEzProviderCfg {
    /// Base URL (`""` = bundled overlay sentinel upstream).
    #[serde(default)]
    pub base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<EzSecretInput>,
    /// e.g. `openai-completions` (default for our 3).
    #[serde(default = "default_api")]
    pub api: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
    /// Minimal model list (full catalog merge deferred).
    #[serde(default)]
    pub models: Vec<RustEzModelDef>,
}

fn default_api() -> String {
    "openai-completions".to_string()
}

/// Minimal model entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustEzModelDef {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Channels v1: discord only (telegram+rest in TODO, types deferred).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RustEzChannelsCfg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discord: Option<RustEzDiscordCfg>,
}

/// Discord subset (upstream `DiscordAccountConfig` trimmed).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RustEzDiscordCfg {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<EzSecretInput>,
    /// `pairing|allowlist|open|disabled` (default `pairing`).
    #[serde(default = "default_pairing")]
    pub dm_policy: String,
    #[serde(default)]
    pub allow_from: Vec<EzAllowFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_mention: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_limit: Option<u32>,
}

fn default_true() -> bool {
    true
}

fn default_pairing() -> String {
    "pairing".to_string()
}

/// Memory search subset (Qdrant via `provider:"qdrant"` + `remote.baseUrl`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RustEzMemoryCfg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<RustEzRemoteCfg>,
}

/// Remote (Qdrant `baseUrl` + key).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RustEzRemoteCfg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<EzSecretInput>,
}

/// MCP subset: `command`→stdio, `url`→remote (transport inferred).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RustEzMcpCfg {
    #[serde(default)]
    pub servers: HashMap<String, RustEzMcpServer>,
}

/// Single MCP server (trimmed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustEzMcpServer {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
}

/// Resolve config path: `EZ_CONFIG_PATH` → `RUSTEZ_CONFIG_PATH` → `./rustez.json`.
pub fn rustez_path() -> String {
    crate::compat_path()
}

/// Load `rustez.json` from disk (JSON only in tiny scope; JSON5/`$include`/exec in TODO).
/// Missing file → `Default` (keeps first-run light).
pub fn rustez_load(path: &str) -> anyhow::Result<RustEzConfig> {
    let bytes = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(RustEzConfig::default()),
        Err(e) => anyhow::bail!("read {path}: {e}"),
    };
    if bytes.trim().is_empty() {
        return Ok(RustEzConfig::default());
    }
    serde_json::from_str(&bytes).map_err(|e| anyhow::anyhow!("parse {path}: {e}"))
}

fn compat_path() -> String {
    std::env::var("EZ_CONFIG_PATH")
        .or_else(|_| std::env::var("RUSTEZ_CONFIG_PATH"))
        .unwrap_or_else(|_| "./rustez.json".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_default() {
        let cfg = rustez_load("/nonexistent-rustez-xyz.json").unwrap();
        assert_eq!(cfg.gateway.port, 18790);
        assert_eq!(cfg.gateway.bind, "loopback");
    }

    #[test]
    fn parses_trimmed_json() {
        let v: RustEzConfig = serde_json::from_str(
            r#"{"gateway":{"port":18790,"mode":"local","bind":"loopback","auth":{"mode":"token"}},
            "channels":{"discord":{"enabled":true,"dmPolicy":"pairing"}}}"#,
        )
        .unwrap();
        assert!(v.channels.discord.unwrap().enabled);
    }
}
