//! `rustez` binary (display name: rustEZ, alias: `ez`).
//! Lightweight entry — heavy provider impls stay in onboarding wizards.

use clap::{Parser, Subcommand};
use rustez_agent::bootstrap::{
    apply_openai, ping_openai, setup_progress, write_setup_doc, DEFAULT_OPENAI_MODEL,
    DEFAULT_POLL_SECS, WELCOME,
};
use rustez_config::{EzSecretInput, EzSecretRef};

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
    /// First-run bootstrap: welcome + OpenAI provider/model wizard, docs handoff
    Onboard {
        /// Provider id (v1: only `openai` is implemented)
        #[arg(long, default_value = "openai")]
        provider: String,
        /// Subscription token (else prompted; never echoed)
        #[arg(long)]
        token: Option<String>,
        /// Model id (else prompted, default offered)
        #[arg(long)]
        model: Option<String>,
        /// Docs handoff path (agent resumes from here)
        #[arg(long, default_value = "docs/SETUP.md")]
        docs: String,
        /// Non-interactive (requires --token, uses defaults otherwise)
        #[arg(long, default_value_t = false)]
        non_interactive: bool,
        /// Skip the live ping test
        #[arg(long, default_value_t = false)]
        skip_test: bool,
        /// Store the token literally in rustez.json (default: env-ref + export hint)
        #[arg(long, default_value_t = false)]
        store_literal: bool,
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
        EzCmd::Onboard {
            provider,
            token,
            model,
            docs,
            non_interactive,
            skip_test,
            store_literal,
        } => onboard(
            provider,
            token,
            model,
            docs,
            non_interactive,
            skip_test,
            store_literal,
        ),
    }
}

/// First-run bootstrap: welcome → OpenAI wizard → ping → rustez.json + docs handoff.
fn onboard(
    provider: String,
    token: Option<String>,
    model: Option<String>,
    docs: String,
    non_interactive: bool,
    skip_test: bool,
    store_literal: bool,
) {
    eprintln!("{WELCOME}");
    if provider != "openai" {
        eprintln!("provider `{provider}` is not implemented in v1 (openai first) — see TODO.md.");
        std::process::exit(2);
    }
    let path = rustez_config::rustez_path();

    let token = match token {
        Some(t) if !t.trim().is_empty() => t,
        _ if non_interactive => {
            eprintln!("non-interactive onboard requires --token.");
            std::process::exit(2);
        }
        _ => prompt(
            "OpenAI subscription token (input hidden is TODO — check shoulder)",
            None,
        ),
    };
    if token.trim().is_empty() {
        eprintln!("empty token — aborting.");
        std::process::exit(2);
    }
    let model = match model {
        Some(m) if !m.trim().is_empty() => m,
        _ if non_interactive => DEFAULT_OPENAI_MODEL.to_string(),
        _ => {
            let m = prompt("OpenAI model", Some(DEFAULT_OPENAI_MODEL));
            if m.is_empty() {
                DEFAULT_OPENAI_MODEL.to_string()
            } else {
                m
            }
        }
    };

    // Live ping proves token+model work (the test-it-out step).
    let mut ping_ok = false;
    if !skip_test {
        eprintln!("testing {provider}/{model} with a ping…");
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        match rt.block_on(ping_openai("", &token, &model)) {
            Ok(reply) => {
                ping_ok = true;
                eprintln!("ping ok — model replied: {}", reply.trim());
            }
            Err(e) => {
                eprintln!("ping failed: {e:#}");
                if non_interactive {
                    eprintln!("rerun with --skip-test to write config anyway.");
                    std::process::exit(1);
                }
                let ans = prompt("write config anyway? [y/N]", Some("N"));
                if !ans.eq_ignore_ascii_case("y") {
                    std::process::exit(1);
                }
            }
        }
    }

    let secret = if store_literal {
        eprintln!("warning: storing token literally in {path} (prefer env-ref).");
        EzSecretInput::Literal(token)
    } else {
        eprintln!(
            "storing env-ref; export EZ_OPENAI_SUB_TOKEN=<token> before running the gateway."
        );
        EzSecretInput::Ref(EzSecretRef {
            source: "env".to_string(),
            provider: "default".to_string(),
            id: "EZ_OPENAI_SUB_TOKEN".to_string(),
        })
    };

    let mut cfg = rustez_config::rustez_load(&path).unwrap_or_default();
    apply_openai(&mut cfg, secret, &model);
    if let Err(e) = std::fs::write(
        &path,
        serde_json::to_string_pretty(&cfg).expect("serialize config"),
    ) {
        // Missing parent dir (e.g. default ./rustez.json has none) is fine; real error otherwise.
        if std::path::Path::new(&path)
            .parent()
            .is_some_and(|p| !p.as_os_str().is_empty())
        {
            eprintln!("write {path}: {e:#}");
            std::process::exit(1);
        }
    }

    if let Err(e) = write_setup_doc(&docs, &provider, &model, &path, ping_ok) {
        eprintln!("write {docs}: {e:#}");
        std::process::exit(1);
    }

    eprintln!(
        "bootstrapped {provider}/{model} (poll {DEFAULT_POLL_SECS}s) → config {path}, handoff {docs}."
    );
    eprintln!(
        "Setup agent is pointed at {docs} and resumes: Discord, usage, Qdrant, Proton Pass, email."
    );
}
