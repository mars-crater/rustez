# RustEZ task runner (just 1.21.0) — thin wrappers over cargo, keeps things light.
# Usage: just build | just test | just run gateway | just wiz proton-pass | just smoke

default: check

# fast typecheck, no binary
check:
    cargo check --workspace

# debug binary -> ./target/debug/rustez
build:
    cargo build -p rustez

# optimized binary -> ./target/release/rustez
release:
    cargo build --release -p rustez

# format (fail-first in CI)
fmt:
    cargo fmt --check

# lint deny warnings (upper_case_acronyms, unsafe_code)
lint:
    cargo clippy --all-targets -- --deny warnings

# workspace tests (unit + e2e)
test:
    cargo test --workspace

# e2e only: tests/e2e_*.rs (health/config/wizards/cli/traits)
e2e:
    cargo test --test 'e2e_*'

# run binary with args, e.g. just run gateway
run *ARGS:
    cargo run -p rustez -- {{ARGS}}

# config doctor (port/mode/discord/providers + setup progress)
doctor:
    cargo run -p rustez -- doctor

# first-run bootstrap: welcome + OpenAI wizard + docs handoff
onboard *ARGS:
    cargo run -p rustez -- onboard {{ARGS}}

# serve gateway on 127.0.0.1:{port} (default 18790)
gateway:
    cargo run -p rustez -- gateway

# show wizard schema for one impl, e.g. just wiz discord
wiz KEY="discord":
    curl -s http://127.0.0.1:18790/api/wizards/{{KEY}} | head -c 2000; echo

# end-to-end smoke: build + doctor + boot + health/config/wizard probes
smoke:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build -p rustez
    ./target/debug/rustez doctor
    ./target/debug/rustez gateway & SRV=$!; trap 'kill $SRV 2>/dev/null || true' EXIT; sleep 1.2
    curl -sf http://127.0.0.1:18790/healthz
    echo
    curl -sf http://127.0.0.1:18790/api/wizards/discord | grep -q '"supported":true'
    echo SMOKE_OK

# install binary to ~/.cargo/bin/rustez
install:
    cargo install --path . --bin rustez

# wipe target/
clean:
    cargo clean
