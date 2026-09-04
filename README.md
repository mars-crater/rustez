# RustEZ

Lightweight, blazing-fast OpenClaw-compatible agent gateway in Rust.

## Install

```bash
# Rust + task runner
rustup default stable
sudo apt install just  # or: cargo install just

git clone https://github.com/mars-crater/rustez.git
cd rustez
just build
```

Optional: `just install` puts `rustez` on `~/.cargo/bin`.

## Run

```bash
# First run: welcome + OpenAI wizard, writes rustez.json + docs/SETUP.md
just onboard
# Non-interactive:
just onboard -- --token $EZ_OPENAI_SUB_TOKEN --model gpt-5.6-sol --non-interactive

# Start the gateway (127.0.0.1:18790)
just gateway

# Check health / config
just doctor
curl -s localhost:18790/healthz
```

## Test

```bash
just test   # unit + e2e
just e2e    # e2e only (health/config/wizards/cli/traits)
just lint   # clippy, warnings denied
```

See `REPORT.md` (work ledger) and `TODO.md` (deferred scope).
Agents: start with `docs/AGENT.md` — the shared manual for working with RustEZ.
