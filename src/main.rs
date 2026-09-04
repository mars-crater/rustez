//! `rustez` binary (display name: rustEZ, alias: `ez`).
//! Lightweight entry — CLI parsing only, heavy impls deferred to onboarding wizards.

use clap::{Parser, Subcommand};

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
    /// Run gateway (stub — full serve lands after onboarding specs)
    Gateway,
    /// Config doctor (stub)
    Doctor,
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
                Ok(cfg) => println!(
                    "doctor ok — gateway {}:{} — discord {} — providers openai:{} opencode-go:{} chutes:{}",
                    cfg.gateway.mode,
                    cfg.gateway.port,
                    cfg.channels.discord.is_some(),
                    cfg.providers.openai.is_some(),
                    cfg.providers.opencode_go.is_some(),
                    cfg.providers.chutes.is_some(),
                ),
                Err(e) => {
                    eprintln!("doctor fail: {e:#}");
                    std::process::exit(1);
                }
            }
        }
    }
}
