//! `rustez` binary (display name: rustEZ, alias: `ez`).
//! Lightweight entry — heavy provider impls stay in onboarding wizards.

use clap::{Parser, Subcommand};
use rustez_agent::bootstrap::{
    apply_openai, setup_progress, write_setup_doc, DEFAULT_OPENAI_MODEL, DEFAULT_POLL_SECS, WELCOME,
};
use rustez_agent::oauth;

mod tui;

#[derive(Parser)]
#[command(
    name = "rustez",
    bin_name = "rustez",
    about = "RustEZ — lightweight agent gateway"
)]
struct EzCli {
    #[command(subcommand)]
    cmd: EzCmd,
}

#[derive(Subcommand)]
enum EzCmd {
    /// Show status (gateway/config stub)
    Status,
    /// Run gateway (serves health/config/wizard schemas)
    Gateway,
    /// Config doctor (validates + reports setup progress)
    Doctor,
    /// ChatGPT OAuth login / status / logout (browser dance, no token to paste)
    Auth {
        #[command(subcommand)]
        cmd: EzAuthCmd,
    },
    /// First-run bootstrap: welcome + OpenAI dance, docs handoff
    Onboard {
        /// Provider id (v1: only `openai` is implemented)
        #[arg(long, default_value = "openai")]
        provider: String,
        /// Model id (else prompted, default offered)
        #[arg(long)]
        model: Option<String>,
        /// Docs handoff path (agent resumes from here)
        #[arg(long, default_value = "docs/SETUP.md")]
        docs: String,
        /// Device-code flow (headless / no localhost browser return)
        #[arg(long, default_value_t = false)]
        device: bool,
        /// Skip the localhost callback: print the URL and paste the code/redirect instead
        #[arg(long, default_value_t = false)]
        paste: bool,
        /// Print the authorize URL + verifier/state as JSON and exit (scripted handoff)
        #[arg(long, default_value_t = false)]
        print_url: bool,
        /// Pasted code or full redirect URL (pairs with --verifier; non-interactive)
        #[arg(long)]
        paste_code: Option<String>,
        /// Code verifier matching --paste-code (from a prior --print-url)
        #[arg(long)]
        verifier: Option<String>,
        /// Don't spawn a browser (just print the URL)
        #[arg(long, default_value_t = false)]
        no_open: bool,
        /// Non-interactive (requires --paste-code + --verifier)
        #[arg(long, default_value_t = false)]
        non_interactive: bool,
        /// Skip the live codex ping test
        #[arg(long, default_value_t = false)]
        skip_test: bool,
    },
}

#[derive(Subcommand)]
enum EzAuthCmd {
    /// Run the OAuth dance and store credentials (0600)
    Login {
        /// Provider id (v1: only `openai`)
        #[arg(long, default_value = "openai")]
        provider: String,
        /// Device-code flow (headless)
        #[arg(long, default_value_t = false)]
        device: bool,
        /// Paste code/URL instead of localhost callback
        #[arg(long, default_value_t = false)]
        paste: bool,
        /// Don't spawn a browser
        #[arg(long, default_value_t = false)]
        no_open: bool,
    },
    /// Show stored auth state (never prints secrets)
    Status {
        /// Provider id (v1: only `openai`)
        #[arg(long, default_value = "openai")]
        provider: String,
    },
    /// Delete stored credentials
    Logout {
        /// Provider id (v1: only `openai`)
        #[arg(long, default_value = "openai")]
        provider: String,
    },
}

/// Prompt on stdin (returns `default` on empty line). Stderr so pipes stay clean.
fn prompt(label: &str, default: Option<&str>) -> String {
    use std::io::{IsTerminal, Write};
    match default {
        Some(d) => eprint!("{label} [{d}]: "),
        None => eprint!("{label}: "),
    }
    let _ = std::io::stderr().flush();
    if !std::io::stdin().is_terminal() {
        return default.unwrap_or("").to_string();
    }
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return default.unwrap_or("").to_string();
    }
    let t = line.trim().to_string();
    if t.is_empty() {
        default.unwrap_or("").to_string()
    } else {
        t
    }
}

fn main() {
    tracing_subscriber::fmt::init();
    let cli = EzCli::parse();
    match cli.cmd {
        EzCmd::Status => {
            let path = rustez_config::rustez_path();
            let cfg = rustez_config::rustez_load(&path).unwrap_or_default();
            println!("rustez 0.1.0 — port {} — config {path}", cfg.gateway.port);
        }
        EzCmd::Gateway => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            rt.block_on(async {
                let path = rustez_config::rustez_path();
                let cfg = rustez_config::rustez_load(&path).unwrap_or_default();
                if cfg.providers.openai.is_none() && cfg.providers.opencode_go.is_none() {
                    eprintln!("{WELCOME}Run `rustez onboard --provider openai` to begin.");
                }
                let gw = rustez_gateway::EzGateway::new(cfg.gateway.port);
                println!(
                    "rustez gateway on 127.0.0.1:{} (config {path})",
                    cfg.gateway.port
                );
                if let Err(e) = gw.serve().await {
                    eprintln!("gateway error: {e:#}");
                    std::process::exit(1);
                }
            });
        }
        EzCmd::Doctor => {
            let path = rustez_config::rustez_path();
            match rustez_config::rustez_load(&path) {
                Ok(cfg) => {
                    let setup = setup_progress("docs/SETUP.md")
                        .map(|(d, t)| format!(" — setup {d}/{t} (docs/SETUP.md)"))
                        .unwrap_or_default();
                    println!(
                        "doctor ok — gateway {}:{} — discord {} — providers openai:{} opencode-go:{} chutes:{}{setup}",
                        cfg.gateway.mode,
                        cfg.gateway.port,
                        cfg.channels.discord.is_some(),
                        cfg.providers.openai.is_some(),
                        cfg.providers.opencode_go.is_some(),
                        cfg.providers.chutes.is_some(),
                    );
                }
                Err(e) => {
                    eprintln!("doctor fail: {e:#}");
                    std::process::exit(1);
                }
            }
        }
        EzCmd::Auth { cmd } => match cmd {
            EzAuthCmd::Login {
                provider,
                device,
                paste,
                no_open,
            } => {
                require_openai(&provider);
                let profile = run_dance(&DanceOpts {
                    device,
                    paste,
                    print_url: false,
                    paste_code: None,
                    verifier: None,
                    no_open,
                });
                eprintln!("logged in: {}", describe_profile(&profile));
            }
            EzAuthCmd::Status { provider } => {
                require_openai(&provider);
                match oauth::load_profile(&provider).expect("read auth store") {
                    Some(p) => println!("auth {provider}: {}", describe_profile(&p)),
                    None => println!("auth {provider}: not logged in — run `rustez auth login`"),
                }
            }
            EzAuthCmd::Logout { provider } => {
                require_openai(&provider);
                if oauth::clear_profile(&provider).expect("clear auth store") {
                    println!("auth {provider}: logged out");
                } else {
                    println!("auth {provider}: was not logged in");
                }
            }
        },
        EzCmd::Onboard {
            provider,
            model,
            docs,
            device,
            paste,
            print_url,
            paste_code,
            verifier,
            no_open,
            non_interactive,
            skip_test,
        } => onboard(OnboardOpts {
            provider,
            model,
            docs,
            device,
            paste,
            print_url,
            paste_code,
            verifier,
            no_open,
            non_interactive,
            skip_test,
        }),
    }
}

/// v1 supports the ChatGPT OAuth dance for `openai` only.
fn require_openai(provider: &str) {
    if provider != "openai" {
        eprintln!("provider `{provider}` is not implemented in v1 (openai first) — see TODO.md.");
        std::process::exit(2);
    }
}

/// One-line, secret-free profile summary.
fn describe_profile(p: &oauth::EzAuthProfile) -> String {
    let left = p.expires_at_ms.saturating_sub(oauth::now_ms()) / 1000;
    format!(
        "{} (account {}, access expires in {}s)",
        if p.email.is_empty() {
            "unknown email"
        } else {
            &p.email
        },
        if p.account_id.is_empty() {
            "unknown"
        } else {
            &p.account_id
        },
        left
    )
}

/// Dance options shared by `auth login` and `onboard`.
struct DanceOpts {
    device: bool,
    paste: bool,
    print_url: bool,
    paste_code: Option<String>,
    verifier: Option<String>,
    no_open: bool,
}

/// Run the ChatGPT OAuth dance and persist `{access, refresh, expires, account}`.
/// No token is ever pasted, printed, or logged — only the browser flow, the
/// device flow, or a pasted redirect carrying a one-time code.
fn run_dance(o: &DanceOpts) -> oauth::EzAuthProfile {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    rt.block_on(run_dance_async(o))
}

async fn run_dance_async(o: &DanceOpts) -> oauth::EzAuthProfile {
    // Scripted handoff: exchange a pasted code with a prior --print-url verifier.
    if let Some(code_in) = o.paste_code.as_deref() {
        let verifier = o.verifier.as_deref().unwrap_or("").trim();
        if verifier.is_empty() {
            eprintln!("--paste-code needs --verifier from a prior --print-url.");
            std::process::exit(2);
        }
        let (code, _) = oauth::parse_pasted_code(code_in).unwrap_or_else(|e| {
            eprintln!("bad pasted code: {e:#}");
            std::process::exit(1);
        });
        return finish_login(
            oauth::exchange_code(&code, verifier, &oauth::redirect_uri(), None).await,
        );
    }
    if o.device {
        return device_dance().await;
    }
    browser_dance(o.paste, o.print_url, o.no_open).await
}

/// Print URL + verifier/state for a later `--paste-code` (stdout = scriptable).
fn print_url_handoff() -> ! {
    let (verifier, challenge) = oauth::pkce_pair();
    let state = oauth::new_state();
    let uri = oauth::redirect_uri();
    println!(
        "{}",
        serde_json::json!({
            "url": oauth::authorize_url(&uri, &challenge, &state),
            "verifier": verifier,
            "state": state,
            "redirect_uri": uri,
        })
    );
    std::process::exit(0);
}

/// Browser dance: open `auth.openai.com`, catch the `:1455` callback (or pasted URL).
async fn browser_dance(paste: bool, print_url: bool, no_open: bool) -> oauth::EzAuthProfile {
    if let Err(e) = oauth::preflight().await {
        eprintln!("{e:#}");
        std::process::exit(1);
    }
    let (verifier, challenge) = oauth::pkce_pair();
    let state = oauth::new_state();
    let uri = oauth::redirect_uri();
    let url = oauth::authorize_url(&uri, &challenge, &state);
    if print_url {
        print_url_handoff();
    }
    eprintln!("opening the browser at auth.openai.com — sign in with your ChatGPT account.");
    eprintln!("{url}");
    if !no_open {
        oauth::open_browser(&url);
    }
    if paste {
        let pasted = prompt("Paste the code or full redirect URL", None);
        let (code, got_state) = oauth::parse_pasted_code(&pasted).unwrap_or_else(|e| {
            eprintln!("bad pasted code: {e:#}");
            std::process::exit(1);
        });
        if let Some(s) = got_state {
            if s != state {
                eprintln!("state mismatch — refusing (possible CSRF). Restart the dance.");
                std::process::exit(1);
            }
        }
        return finish_login(oauth::exchange_code(&code, &verifier, &uri, None).await);
    }
    eprintln!("waiting on http://localhost:1455/auth/callback (up to 5 min)…");
    match oauth::wait_for_callback(&state, std::time::Duration::from_secs(300)) {
        Ok(code) => finish_login(oauth::exchange_code(&code, &verifier, &uri, None).await),
        Err(e) => {
            eprintln!("callback failed: {e:#}");
            eprintln!("If the browser approved but localhost:1455 shows unreachable,");
            eprintln!(
                "copy the full address-bar URL (it still has ?code=…) and rerun with --paste."
            );
            eprintln!("Headless machine: rerun with --device.");
            std::process::exit(1);
        }
    }
}

/// Device dance for headless machines: code entry at the verification page.
async fn device_dance() -> oauth::EzAuthProfile {
    if let Err(e) = oauth::preflight().await {
        eprintln!("{e:#}");
        std::process::exit(1);
    }
    let (id, code, interval) = oauth::device_usercode().await.unwrap_or_else(|e| {
        eprintln!("device flow rejected: {e:#}");
        std::process::exit(1);
    });
    eprintln!("open {} and enter code: {code}", oauth::DEVICE_VERIFY_URL);
    let (auth_code, verifier) = oauth::device_poll(&id, &code, interval)
        .await
        .unwrap_or_else(|e| {
            eprintln!("device approval failed: {e:#}");
            std::process::exit(1);
        });
    finish_login(
        oauth::exchange_code(&auth_code, &verifier, oauth::DEVICE_REDIRECT_URI, None).await,
    )
}

/// Verify identity, persist `0600`, return the profile.
fn finish_login(res: anyhow::Result<oauth::EzAuthProfile>) -> oauth::EzAuthProfile {
    let profile = res.unwrap_or_else(|e| {
        eprintln!("login failed: {e:#}");
        std::process::exit(1);
    });
    if profile.account_id.is_empty() {
        eprintln!("login failed: token carries no ChatGPT account id.");
        std::process::exit(1);
    }
    oauth::save_profile("openai", &profile).unwrap_or_else(|e| {
        eprintln!("save credentials: {e:#}");
        std::process::exit(1);
    });
    profile
}

/// Onboard knobs (kept in a struct — clippy arg-count).
#[allow(clippy::struct_field_names)]
struct OnboardOpts {
    provider: String,
    model: Option<String>,
    docs: String,
    device: bool,
    paste: bool,
    print_url: bool,
    paste_code: Option<String>,
    verifier: Option<String>,
    no_open: bool,
    non_interactive: bool,
    skip_test: bool,
}

/// Shared onboard inputs, resolved before the flow runs (CLI prompts or TUI screens).
pub(crate) struct FlowParams {
    pub provider: String,
    pub model: String,
    pub docs: String,
    pub device: bool,
    pub paste: bool,
    pub paste_code: Option<String>,
    pub verifier: Option<String>,
    pub no_open: bool,
    pub skip_test: bool,
    /// On ping failure: ask "write config anyway?" (`true`) or abort (`false`).
    pub ask_on_ping_fail: bool,
    /// Used only when `ask_on_ping_fail` is set.
    pub ask: Box<dyn Fn(&str) -> bool + Send>,
}

/// Shared onboard result (secret-free).
pub(crate) struct FlowSummary {
    pub provider: String,
    pub model: String,
    pub email: String,
    pub account_id: String,
    pub config_path: String,
    pub docs_path: String,
    pub ping_reply: Option<String>,
}

/// Core flow both frontends share: dance → ping → rustez.json + docs handoff.
/// Logs progress via `log`; never logs secrets.
pub(crate) fn run_flow(p: &FlowParams, log: &dyn Fn(&str)) -> anyhow::Result<FlowSummary> {
    let path = rustez_config::rustez_path();

    let profile = run_dance(&DanceOpts {
        device: p.device,
        paste: p.paste,
        print_url: false,
        paste_code: p.paste_code.clone(),
        verifier: p.verifier.clone(),
        no_open: p.no_open,
    });
    log(&format!("logged in: {}", describe_profile(&profile)));

    // Live test-it-out: one tiny subscription-backed chat through the codex backend.
    let mut ping_reply = None;
    if !p.skip_test {
        log(&format!("testing {}/{} with a ping…", p.provider, p.model));
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        match rt.block_on(oauth::chat_codex(
            &profile.access_token,
            &profile.account_id,
            &p.model,
            "ping",
        )) {
            Ok(reply) => {
                let short: String = reply.trim().chars().take(120).collect();
                log(&format!("ping ok — model replied: {short}"));
                ping_reply = Some(short);
            }
            Err(e) => {
                if p.ask_on_ping_fail && (p.ask)("write config anyway? [y/N]") {
                    log(&format!("ping failed ({e:#}) — writing config anyway."));
                } else {
                    anyhow::bail!("ping failed: {e:#}");
                }
            }
        }
    }

    // OAuth creds live in the 0600 auth store — config only selects provider+model.
    let mut cfg = rustez_config::rustez_load(&path).unwrap_or_default();
    apply_openai(
        &mut cfg,
        rustez_config::EzSecretInput::Literal(String::new()),
        &p.model,
    );
    cfg.providers.openai.as_mut().expect("just set").api_key = None;
    cfg.providers.openai.as_mut().expect("just set").api = "chatgpt-codex".to_string();
    if let Err(e) = std::fs::write(
        &path,
        serde_json::to_string_pretty(&cfg).expect("serialize config"),
    ) {
        // Missing parent dir (e.g. default ./rustez.json has none) is fine; real error otherwise.
        if std::path::Path::new(&path)
            .parent()
            .is_some_and(|p| !p.as_os_str().is_empty())
        {
            anyhow::bail!("write {path}: {e:#}");
        }
    }

    write_setup_doc(&p.docs, &p.provider, &p.model, &path, ping_reply.is_some())
        .map_err(|e| anyhow::anyhow!("write {}: {e:#}", p.docs))?;

    log(&format!(
        "bootstrapped {}/{} (poll {DEFAULT_POLL_SECS}s) → config {path}, handoff {}.",
        p.provider, p.model, p.docs
    ));
    Ok(FlowSummary {
        provider: p.provider.clone(),
        model: p.model.clone(),
        email: profile.email.clone(),
        account_id: profile.account_id.clone(),
        config_path: path,
        docs_path: p.docs.clone(),
        ping_reply,
    })
}

/// First-run bootstrap dispatcher: TUI when interactive on a TTY, prompts otherwise.
fn onboard(o: OnboardOpts) {
    eprintln!("{WELCOME}");
    require_openai(&o.provider);
    if o.print_url {
        print_url_handoff();
    }
    if !o.non_interactive
        && o.paste_code.is_none()
        && std::io::IsTerminal::is_terminal(&std::io::stdout())
    {
        match tui::run_tui(&o.provider, o.model.as_deref(), &o.docs, o.skip_test) {
            Ok(Some(s)) => {
                eprintln!(
                    "bootstrapped {}/{} → config {}, handoff {}.",
                    s.provider, s.model, s.config_path, s.docs_path
                );
                return;
            }
            Ok(None) => {
                eprintln!("onboard cancelled.");
                return;
            }
            Err(e) => eprintln!("TUI unavailable ({e:#}); falling back to prompts."),
        }
    }
    if o.non_interactive && o.paste_code.is_none() {
        eprintln!("non-interactive onboard needs --paste-code + --verifier (from --print-url),");
        eprintln!("or run interactively: the dance needs a human in the browser.");
        std::process::exit(2);
    }
    let model = match o.model {
        Some(m) if !m.trim().is_empty() => m,
        _ if o.non_interactive => DEFAULT_OPENAI_MODEL.to_string(),
        _ => {
            let m = prompt("OpenAI model", Some(DEFAULT_OPENAI_MODEL));
            if m.is_empty() {
                DEFAULT_OPENAI_MODEL.to_string()
            } else {
                m
            }
        }
    };
    let params = FlowParams {
        provider: o.provider,
        model,
        docs: o.docs,
        device: o.device,
        paste: o.paste,
        paste_code: o.paste_code,
        verifier: o.verifier,
        no_open: o.no_open,
        skip_test: o.skip_test,
        ask_on_ping_fail: !o.non_interactive,
        ask: Box::new(|q| prompt(q, Some("N")).eq_ignore_ascii_case("y")),
    };
    match run_flow(&params, &|m| eprintln!("{m}")) {
        Ok(_) => eprintln!(
            "Setup agent is pointed at {} and resumes: Discord, usage, Qdrant, Proton Pass, email.",
            params.docs
        ),
        Err(e) => {
            eprintln!("onboard failed: {e:#}");
            std::process::exit(1);
        }
    }
}
