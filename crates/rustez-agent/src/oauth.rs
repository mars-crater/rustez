//! ChatGPT OAuth dance for the `openai` provider (no token to paste).
//!
//! Mirrors the upstream OpenClaw/Codex CLI flow against the public Codex client:
//! PKCE S256 → browser authorize on `auth.openai.com` → localhost `:1455`
//! callback (or device-code fallback) → code exchange → `{access, refresh,
//! expires, account}` stored at `0600` → auto-refresh → ChatGPT backend chat.
//!
//! Secrets never hit logs: [`EzAuthProfile`] has a redacted `Debug`.

use std::io::{Read, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::Engine as _;

/// Public Codex CLI client reused by open tooling (no secret — PKCE only).
pub const CHATGPT_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
/// OAuth issuer endpoints.
pub const AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
pub const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
/// Localhost callback (must match what the public client allows).
pub const CALLBACK_PORT: u16 = 1455;
pub const CALLBACK_PATH: &str = "/auth/callback";
/// Scopes: `offline_access` is what yields a refresh token.
pub const SCOPE: &str = "openid profile email offline_access";
/// Device-flow endpoints + verification page.
pub const DEVICE_USERCODE_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/usercode";
pub const DEVICE_TOKEN_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/token";
pub const DEVICE_VERIFY_URL: &str = "https://auth.openai.com/codex/device";
pub const DEVICE_REDIRECT_URI: &str = "https://auth.openai.com/deviceauth/callback";
/// ChatGPT backend (NOT `api.openai.com`) for subscription-backed chat.
pub const CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
/// Refresh this far before expiry.
pub const REFRESH_MARGIN_MS: u128 = 120_000;

/// Stored credential set. Custom `Debug` redacts token material.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct EzAuthProfile {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_ms: u128,
    pub account_id: String,
    #[serde(default)]
    pub email: String,
}

impl std::fmt::Debug for EzAuthProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EzAuthProfile")
            .field("access_token", &"***")
            .field("refresh_token", &"***")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("account_id", &self.account_id)
            .field("email", &self.email)
            .finish()
    }
}

/// Identity decoded from the `id_token` (signature not verified — routing metadata only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EzAuthIdentity {
    pub account_id: String,
    pub email: String,
}

/// Milliseconds since epoch.
pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Auth dir: `$RUSTEZ_AUTH_DIR` or `~/.rustez/auth`.
pub fn auth_dir() -> std::path::PathBuf {
    if let Ok(d) = std::env::var("RUSTEZ_AUTH_DIR") {
        return std::path::PathBuf::from(d);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::Path::new(&home).join(".rustez/auth")
}

/// Profile path for a provider (`openai` → `…/auth/openai.json`).
pub fn profile_path(provider: &str) -> std::path::PathBuf {
    auth_dir().join(format!("{provider}.json"))
}

/// Save with `0600` permissions (dir `0700`).
pub fn save_profile(provider: &str, profile: &EzAuthProfile) -> anyhow::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let dir = auth_dir();
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let body = serde_json::to_string_pretty(profile)?;
    let mut opt = std::fs::OpenOptions::new();
    opt.write(true).create(true).truncate(true).mode(0o600);
    let mut f = opt.open(profile_path(provider))?;
    f.write_all(body.as_bytes())?;
    Ok(())
}

/// Load stored credentials (`None` = never logged in).
pub fn load_profile(provider: &str) -> anyhow::Result<Option<EzAuthProfile>> {
    let path = profile_path(provider);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => anyhow::bail!("read {}: {e}", path.display()),
    };
    Ok(Some(serde_json::from_str(&text).map_err(|e| {
        anyhow::anyhow!("parse {}: {e}", path.display())
    })?))
}

/// Delete stored credentials.
pub fn clear_profile(provider: &str) -> anyhow::Result<bool> {
    match std::fs::remove_file(profile_path(provider)) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => anyhow::bail!("remove profile: {e}"),
    }
}

/// Generate a PKCE pair: `(verifier, challenge)` (S256, base64url no-pad).
pub fn pkce_pair() -> (String, String) {
    use rand::RngCore;
    let mut raw = [0u8; 64];
    rand::rng().fill_bytes(&mut raw);
    let verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw);
    (verifier.clone(), pkce_challenge(&verifier))
}

/// S256 challenge for a verifier (pure — RFC 7636 test vector covered in tests).
pub fn pkce_challenge(verifier: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// Random `state` (32 hex chars) for CSRF protection on the callback.
pub fn new_state() -> String {
    use rand::RngCore;
    let mut raw = [0u8; 16];
    rand::rng().fill_bytes(&mut raw);
    hex_of(&raw)
}

fn hex_of(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0xf) as usize] as char);
    }
    s
}

/// Loopback redirect URI, e.g. `http://localhost:1455/auth/callback`.
pub fn redirect_uri() -> String {
    format!("http://localhost:{CALLBACK_PORT}{CALLBACK_PATH}")
}

/// Browser authorize URL for the dance.
pub fn authorize_url(redirect_uri: &str, challenge: &str, state: &str) -> String {
    use std::fmt::Write;
    let mut q = String::new();
    let pairs = [
        ("response_type", "code"),
        ("client_id", CHATGPT_CLIENT_ID),
        ("redirect_uri", redirect_uri),
        ("scope", SCOPE),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("originator", "rustez"),
    ];
    for (i, (k, v)) in pairs.iter().enumerate() {
        if i > 0 {
            q.push('&');
        }
        let _ = write!(q, "{k}={}", percent_encode(v));
    }
    format!("{AUTHORIZE_URL}?{q}")
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            use std::fmt::Write;
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push(h << 4 | l);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Parse a pasted code or full redirect URL into `(code, state?)`.
pub fn parse_pasted_code(input: &str) -> anyhow::Result<(String, Option<String>)> {
    let t = input.trim();
    if t.is_empty() {
        anyhow::bail!("empty code");
    }
    let query = match t.find('?') {
        Some(i) => &t[i + 1..],
        None if t.starts_with("http") => anyhow::bail!("URL has no query string"),
        None => return Ok((t.to_string(), None)),
    };
    let mut code = None;
    let mut state = None;
    for part in query.split('&') {
        let (k, v) = match part.find('=') {
            Some(i) => (&part[..i], &part[i + 1..]),
            None => continue,
        };
        match k {
            "code" => code = Some(percent_decode(v)),
            "state" => state = Some(percent_decode(v)),
            "error" => anyhow::bail!("provider refused: {}", percent_decode(v)),
            _ => {}
        }
    }
    match code {
        Some(c) if !c.is_empty() => Ok((c, state)),
        _ => anyhow::bail!("no ?code= found in pasted input"),
    }
}

/// One-shot localhost callback server: waits for `?code=&state=` on
/// `127.0.0.1:1455/auth/callback`, validates `state`, returns the code.
pub fn wait_for_callback(expected_state: &str, timeout: Duration) -> anyhow::Result<String> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", CALLBACK_PORT)).map_err(|e| {
        anyhow::anyhow!("bind 127.0.0.1:{CALLBACK_PORT}: {e} (is another dance running?)")
    })?;
    listener.set_nonblocking(true)?;
    let deadline = std::time::Instant::now() + timeout;
    let ok_page = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<h1>RustEZ login complete — return to the terminal.</h1>";
    let err_page = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<h1>Login failed — wrong state or missing code. Check the terminal.</h1>";
    while std::time::Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                let mut buf = vec![0u8; 8192];
                let mut got = 0;
                loop {
                    match stream.read(&mut buf[got..]) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            got += n;
                            if got >= buf.len()
                                || (got >= 4 && buf[..got].windows(4).any(|w| w == b"\r\n\r\n"))
                            {
                                break;
                            }
                        }
                    }
                }
                let head = String::from_utf8_lossy(&buf[..got]);
                let target = head
                    .lines()
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("");
                if target.starts_with(CALLBACK_PATH) {
                    match parse_pasted_code(&format!("http://x{target}")) {
                        Ok((code, state)) if state.as_deref() == Some(expected_state) => {
                            let _ = stream.write_all(ok_page.as_bytes());
                            return Ok(code);
                        }
                        _ => {
                            let _ = stream.write_all(err_page.as_bytes());
                        }
                    }
                } else {
                    let _ = stream.write_all(err_page.as_bytes());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => anyhow::bail!("accept: {e}"),
        }
    }
    anyhow::bail!("timed out waiting for the browser callback — rerun with --device (headless) or --manual-code")
}

/// Best-effort browser open (always print the URL too).
pub fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let prog = "open";
    #[cfg(target_os = "windows")]
    let prog = "cmd";
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let prog = "xdg-open";
    #[cfg(target_os = "windows")]
    let args: &[&str] = &["/c", "start", "", url];
    #[cfg(not(target_os = "windows"))]
    let args: &[&str] = &[url];
    let _ = std::process::Command::new(prog).args(args).spawn();
}

/// Exchange an authorization `code` for tokens. `existing_refresh` is reused when
/// the response rotates none (refresh path).
pub async fn exchange_code(
    code: &str,
    verifier: &str,
    redirect_uri: &str,
    existing_refresh: Option<&str>,
) -> anyhow::Result<EzAuthProfile> {
    exchange_like(
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", verifier),
            ("redirect_uri", redirect_uri),
        ],
        existing_refresh,
    )
    .await
}

/// Refresh an access token (rotating refresh persisted by the caller).
pub async fn refresh_tokens(profile: &EzAuthProfile) -> anyhow::Result<EzAuthProfile> {
    exchange_like(
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", &profile.refresh_token),
        ],
        Some(&profile.refresh_token),
    )
    .await
    .map(|mut p| {
        // Preserve identity the refresh response doesn't repeat.
        if p.account_id.is_empty() {
            p.account_id = profile.account_id.clone();
        }
        if p.email.is_empty() {
            p.email = profile.email.clone();
        }
        p
    })
}

async fn exchange_like(
    extra: &[(&str, &str)],
    existing_refresh: Option<&str>,
) -> anyhow::Result<EzAuthProfile> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let mut form = vec![("client_id", CHATGPT_CLIENT_ID)];
    form.extend_from_slice(extra);
    let res = client.post(TOKEN_URL).form(&form).send().await?;
    let status = res.status();
    let body: serde_json::Value = res.json().await.unwrap_or(serde_json::Value::Null);
    if !status.is_success() {
        anyhow::bail!("token endpoint {status}: {}", redact_token_error(&body));
    }
    let access = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let expires_in = body.get("expires_in").and_then(|v| v.as_u64()).unwrap_or(0);
    if access.is_empty() || expires_in == 0 {
        anyhow::bail!("token endpoint returned no access_token/expires_in");
    }
    let refresh = body
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| existing_refresh.map(|s| s.to_string()))
        .unwrap_or_default();
    if refresh.is_empty() {
        anyhow::bail!("token endpoint returned no refresh_token (need offline_access scope)");
    }
    let identity = body
        .get("id_token")
        .and_then(|v| v.as_str())
        .map(identity_from_token)
        .transpose()?
        .unwrap_or(EzAuthIdentity {
            account_id: String::new(),
            email: String::new(),
        });
    Ok(EzAuthProfile {
        access_token: access,
        refresh_token: refresh,
        expires_at_ms: now_ms() + u128::from(expires_in) * 1000,
        account_id: identity.account_id,
        email: identity.email,
    })
}

/// Short error summary (never includes token material).
fn redact_token_error(body: &serde_json::Value) -> String {
    for key in ["error", "error_description", "detail", "message"] {
        if let Some(s) = body.get(key).and_then(|v| v.as_str()) {
            if !s.is_empty() {
                let mut out: String = s.chars().take(160).collect();
                for token_key in ["refresh_token", "access_token", "code_verifier", "code="] {
                    if let Some(i) = out.find(token_key) {
                        out.truncate(i + token_key.len());
                        out.push_str("…[redacted]");
                    }
                }
                return out;
            }
        }
    }
    "unknown provider error".to_string()
}

/// Decode routing identity from an `id_token` (no signature verify — metadata only).
pub fn identity_from_token(id_token: &str) -> anyhow::Result<EzAuthIdentity> {
    let mut parts = id_token.split('.');
    let (_h, payload, _s) = match (parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(s)) => (h, p, s),
        _ => anyhow::bail!("malformed token (expected 3 segments)"),
    };
    let json: serde_json::Value = {
        let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|e| anyhow::anyhow!("token payload not base64: {e}"))?;
        serde_json::from_slice(&raw)?
    };
    let auth = &json["https://api.openai.com/auth"];
    let account_id = auth
        .get("chatgpt_account_id")
        .and_then(|v| v.as_str())
        .or_else(|| json.get("chatgpt_account_id").and_then(|v| v.as_str()))
        .or_else(|| {
            json.get("organizations")
                .and_then(|o| o.get(0))
                .and_then(|o| o.get("id"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("")
        .to_string();
    if account_id.is_empty() {
        anyhow::bail!("token has no account id (not a ChatGPT-backed token?)");
    }
    let email = json
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Ok(EzAuthIdentity { account_id, email })
}

/// Fresh access token, refreshing + persisting when inside the margin.
/// Returns `(access_token, refreshed)`.
pub async fn ensure_access(provider: &str) -> anyhow::Result<(String, bool)> {
    let mut profile = load_profile(provider)?.ok_or_else(|| {
        anyhow::anyhow!("not logged in — run `rustez auth login --provider {provider}`")
    })?;
    if now_ms() + REFRESH_MARGIN_MS < profile.expires_at_ms {
        return Ok((profile.access_token.clone(), false));
    }
    let next = refresh_tokens(&profile).await?;
    profile = next;
    save_profile(provider, &profile)?;
    Ok((profile.access_token.clone(), true))
}

/// Minimal subscription-backed chat via the ChatGPT backend (Responses API, SSE).
pub async fn chat_codex(
    access_token: &str,
    account_id: &str,
    model: &str,
    input: &str,
) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;
    let res = client
        .post(format!("{CODEX_BASE_URL}/responses"))
        .bearer_auth(access_token)
        .header("ChatGPT-Account-Id", account_id)
        .header("originator", "rustez")
        .header("Accept", "text/event-stream")
        .json(&serde_json::json!({
            "model": model,
            "input": [{"role": "user", "content": input}],
            "stream": true,
            "store": false,
        }))
        .send()
        .await?;
    let status = res.status();
    let text = res.text().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!(
            "codex backend {status}: {}",
            redact_token_error(&serde_json::from_str(&text).unwrap_or(serde_json::Value::Null))
        );
    }
    Ok(collect_sse_text(&text))
}

/// Accumulate `response.output_text.delta` text from an SSE body (pure, testable).
pub fn collect_sse_text(body: &str) -> String {
    let mut out = String::new();
    for line in body.lines() {
        let data = match line.strip_prefix("data:") {
            Some(d) => d.trim(),
            None => continue,
        };
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("type").and_then(|t| t.as_str()) == Some("response.output_text.delta") {
            if let Some(d) = v.get("delta").and_then(|d| d.as_str()) {
                out.push_str(d);
            }
        }
        if v.get("type").and_then(|t| t.as_str()) == Some("response.completed") {
            break;
        }
    }
    out
}

/// Device flow, step 1: request a user code.
pub async fn device_usercode() -> anyhow::Result<(String, String, u64)> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let res = client
        .post(DEVICE_USERCODE_URL)
        .header("originator", "rustez")
        .json(&serde_json::json!({"client_id": CHATGPT_CLIENT_ID}))
        .send()
        .await?;
    let body: serde_json::Value = res.json().await.unwrap_or(serde_json::Value::Null);
    let id = body
        .get("device_auth_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let code = body
        .get("user_code")
        .or_else(|| body.get("usercode"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let interval = body
        .get("interval")
        .and_then(|v| v.as_u64())
        .unwrap_or(5)
        .max(1);
    if id.is_empty() || code.is_empty() {
        anyhow::bail!("device flow rejected: {}", redact_token_error(&body));
    }
    Ok((id, code, interval))
}

/// Device flow, step 2: poll until approved (403/404 = pending). Returns
/// `(authorization_code, code_verifier)` for the normal exchange.
pub async fn device_poll(
    device_auth_id: &str,
    user_code: &str,
    interval: u64,
) -> anyhow::Result<(String, String)> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(15 * 60);
    loop {
        if std::time::Instant::now() > deadline {
            anyhow::bail!("device approval timed out (15 min)");
        }
        let res = client
            .post(DEVICE_TOKEN_URL)
            .header("originator", "rustez")
            .json(&serde_json::json!({
                "device_auth_id": device_auth_id,
                "user_code": user_code,
            }))
            .send()
            .await?;
        match res.status().as_u16() {
            // Pending approval — keep polling.
            403 | 404 => {
                tokio::time::sleep(Duration::from_secs(interval)).await;
                continue;
            }
            s => {
                let body: serde_json::Value = res.json().await.unwrap_or(serde_json::Value::Null);
                if !(200..300).contains(&s) {
                    anyhow::bail!("device poll {s}: {}", redact_token_error(&body));
                }
                let code = body
                    .get("authorization_code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let verifier = body
                    .get("code_verifier")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if code.is_empty() || verifier.is_empty() {
                    anyhow::bail!("device poll returned no authorization_code/code_verifier");
                }
                return Ok((code, verifier));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_rfc7636_vector() {
        // RFC 7636 Appendix B test vector.
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let (v, c) = pkce_pair();
        assert_eq!(c, pkce_challenge(&v));
        assert!(v.len() >= 86 && c.len() == 43);
    }

    #[test]
    fn authorize_url_has_dance_params() {
        let url = authorize_url(&redirect_uri(), "CHAL", "STATE");
        for need in [
            "response_type=code",
            "client_id=app_EMoamEEZ73f0CkXaXp7hrann",
            "redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback",
            "scope=openid%20profile%20email%20offline_access",
            "code_challenge=CHAL",
            "code_challenge_method=S256",
            "state=STATE",
            "codex_cli_simplified_flow=true",
        ] {
            assert!(url.contains(need), "missing {need}: {url}");
        }
    }

    #[test]
    fn paste_parsing() {
        assert_eq!(
            parse_pasted_code("abc123").unwrap(),
            ("abc123".to_string(), None)
        );
        let (code, state) =
            parse_pasted_code("http://localhost:1455/auth/callback?code=ZZ9&state=SS2").unwrap();
        assert_eq!((code, state), ("ZZ9".to_string(), Some("SS2".to_string())));
        assert!(parse_pasted_code("   ").is_err());
        assert!(
            parse_pasted_code("http://localhost:1455/auth/callback?error=access_denied").is_err()
        );
    }

    #[test]
    fn identity_from_crafted_token() {
        use base64::engine::Engine;
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            r#"{"https://api.openai.com/auth":{"chatgpt_account_id":"acc-9"},"email":"a@b.c"}"#,
        );
        let id = identity_from_token(&format!("h.{payload}.s")).unwrap();
        assert_eq!(
            id,
            EzAuthIdentity {
                account_id: "acc-9".to_string(),
                email: "a@b.c".to_string(),
            }
        );
        assert!(identity_from_token("not-a-token").is_err());
    }

    #[test]
    fn sse_accumulates_deltas() {
        let body =
            "event: x\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"Hel\"}\n\n\
            data: {\"type\":\"response.output_text.delta\",\"delta\":\"lo\"}\n\n\
            data: {\"type\":\"response.completed\"}\n\ndata: [DONE]\n";
        assert_eq!(collect_sse_text(body), "Hello");
        assert_eq!(collect_sse_text("data: garbage\n"), "");
    }

    #[test]
    fn store_roundtrip_private() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("RUSTEZ_AUTH_DIR", dir.path());
        let p = EzAuthProfile {
            access_token: "a".to_string(),
            refresh_token: "r".to_string(),
            expires_at_ms: 1,
            account_id: "acc".to_string(),
            email: "e".to_string(),
        };
        save_profile("openai", &p).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(profile_path("openai"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
        let back = load_profile("openai").unwrap().unwrap();
        assert_eq!(back.account_id, "acc");
        let dbg = format!("{back:?}");
        assert!(dbg.contains("***"));
        assert!(!dbg.contains("\"a\""));
        assert!(clear_profile("openai").unwrap());
        assert!(load_profile("openai").unwrap().is_none());
        std::env::remove_var("RUSTEZ_AUTH_DIR");
    }
}
