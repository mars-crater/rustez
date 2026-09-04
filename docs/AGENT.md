# RustEZ — Agent Guide

> Read this first. It tells any agent what RustEZ is, how to work with it,
> and how to resume setup. Human quickstart lives in `README.md`.

## 1. What this is

RustEZ (`rustez` binary, display name `RustEZ`) is a lightweight, blazing-fast,
OpenClaw-compatible agent gateway in Rust. Principles that decide everything:
**lightweight, fast, intuitive, simple.** If a change adds weight without need, it
goes to `TODO.md`, not the codebase.

- Source of behavior truth: `github.com/openclaw/openclaw` (types mirrored, never forked).
- Canonical state: `rustez.json` + `docs/SETUP.md` (per-machine, gitignored).
- This `docs/` folder is the shared manual — committed, for anyone's agent.

## 2. Repo map

```
src/main.rs                  # rustez binary: status|gateway|doctor|onboard
crates/rustez-config/        # trimmed schemas: gateway, discord, 3 providers, memory, mcp
crates/rustez-compat/        # ONLY place that may say claw/openclaw (key/env/wire shims)
crates/rustez-gateway/       # axum serve: /healthz, /api/config, /api/wizards/:key
crates/rustez-agent/         # EzProvider trait + bootstrap (welcome, ping, SETUP writer)
crates/rustez-channels/      # EzChannel trait (v1 impl: discord, via onboarding)
crates/rustez-memory/        # EzMemory trait (v1 backend: qdrant, via onboarding)
crates/rustez-voice/         # EzTts/EzStt traits (local-CLI first)
crates/rustez-images/        # EzImageGen trait (file-out first)
crates/rustez-secrets/       # EzSecretStore trait, BYO (proton-pass first)
crates/rustez-email/         # EzEmail trait (smtp first, allowlisted)
crates/rustez-usage/         # EzSubUsage subscription-% (spendings deferred)
crates/rustez-mcp/           # MCP client stub · crates/rustez-cli (helpers)
tests/e2e_*.rs               # per-functionality e2e (health/config/wizards/cli/traits)
ui/src/ez-config/            # focused-node console UI (planned, see §7)
justfile · REPORT.md (ledger) · TODO.md (deferred scope)
```

## 3. Commands (via `just`, just 1.21+)

| Task    | Command                                      |
| ------- | -------------------------------------------- |
| Check   | `just check` (fast typecheck)                |
| Build   | `just build` → `./target/debug/rustez`       |
| Release | `just release` → `./target/release/rustez`   |
| Test    | `just test` (unit + e2e) / `just e2e` (e2e)  |
| Lint    | `just lint` (clippy, warnings denied)        |
| Format  | `cargo fmt` (must be clean)                  |
| Onboard | `just onboard` (TUI on a TTY; flags otherwise — see §7) |
| Serve   | `just gateway` (127.0.0.1:18790) / `just doctor` / `just wiz <key>` |

Gate before any commit: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test --workspace`, `git diff --check`.

## 4. Config (`rustez.json`, trimmed tiny scope)

Resolved path: `EZ_CONFIG_PATH` → `RUSTEZ_CONFIG_PATH` → `./rustez.json`.
Missing/empty file = defaults (gateway `local:18790`). Unknown keys pass through
(`extra` map) — never drop what you don't understand.

- `gateway{port, mode: local|remote, bind: loopback, auth{mode: token, token?}}`
- `channels.discord{enabled, token, dmPolicy: pairing, allowFrom[], requireMention?, historyLimit?}`
- `providers.{openai|opencode-go|chutes}{baseUrl, apiKey?, api: openai-completions, timeoutSeconds?, models[{id}]}`
- `memory{provider: qdrant, model?, remote{baseUrl, apiKey?}}`
- `mcp.servers{name: {enabled, command?/args?, url?/transport?}}`

Secrets: `string | {source: env|file|exec|store, provider, id}`. Prefer
`{source:"env",provider:"default",id:"VAR_NAME"}` over literals. Empty literal = missing.
`GET /api/config` masks literals to `***` — rely on that, never log values.

ChatGPT OAuth (the `openai` dance — there is no token to paste):
`rustez auth login` opens `auth.openai.com` (PKCE S256, public Codex client,
`localhost:1455` callback; `--device` for headless, `--paste` for manual URL paste).
Creds land in `~/.rustez/auth/openai.json` (`0600`: access, rotating refresh,
expiry, account id). The browser dance also saves a pending verifier
(`.pending-openai.json`, `0600`, 30-min use window) so a pasted address-bar URL
finishes the same dance with `--paste-code '<whole-url>'` — no re-approval.
Runtime auto-refreshes inside a 2-min margin and chats via
`chatgpt.com/backend-api/codex` (Bearer + `ChatGPT-Account-Id`). `auth status`
shows state without secrets; `auth logout` deletes. `RUSTEZ_AUTH_DIR` overrides
the store path (tests use this).

## 5. Gateway API (port 18790; Node OpenClaw keeps 18789)

| Endpoint               | Purpose                                              |
| ---------------------- | ---------------------------------------------------- |
| `GET /healthz`         | `{ok, version}` liveness                             |
| `GET /api/config`      | masked runtime config                                |
| `GET /api/wizards/:key`| auth+config field schema for one impl (see §6)       |

Planned (TODO): `GET /api/config/schema`, `PATCH /api/config`,
`POST /api/wizards/:key/test|apply`.

## 6. Wizards (auth + config per impl)

Each supported key returns 2 steps (`auth` then `config`); unsupported keys return
`supported:false`. Auth fields carry `env_hint` — set the env var, don't paste
secrets into chat when avoidable.

| Key           | Auth (env)                          | Config fields                          |
| ------------- | ----------------------------------- | -------------------------------------- |
| `discord`     | token → `EZ_DISCORD_TOKEN`          | dmPolicy/allowFrom/requireMention/historyLimit |
| `openai`      | browser dance, nothing to paste     | model/pollSecs                         |
| `opencode-go` | apiToken → `EZ_OPENCODEGO_TOKEN`    | baseUrl/models                         |
| `chutes`      | apiToken → `EZ_CHUTES_TOKEN`        | baseUrl/models                         |
| `qdrant`      | apiKey? → `RUSTEZ_QDRANT_KEY`       | url/collection/embedModel              |
| `proton-pass` | automizerToken → `EZ_PROTONPASS_AUTOMIZER_TOKEN` | storeId                   |
| `email`       | smtpUser/smtpPass → `EZ_SMTP_USER/PASS` | host/port/allowTo                  |

## 7. Resume protocol (how setup continues)

1. `rustez onboard` opens an interactive TUI on a TTY (welcome → provider →
   method [browser|device|paste] → model → confirm → live dance → done);
   with flags/pipes it falls back to prompts (`--device/--paste/--print-url`,
   `--non-interactive` needs `--paste-code + --verifier`). Either path writes
   `rustez.json` + `docs/SETUP.md` (front-matter
   `provider/model/config/status` + `- [ ]` checklist + Agent section).
2. The setup agent (`openai/<model>`, system prompt in `SETUP.md`) reads this guide,
   then `docs/SETUP.md` and the config it points to, works the next unchecked item
   via its wizard, and checks it off. Order: Discord → usage → Qdrant → Proton Pass → email.
3. `doctor` reports `setup d/t` progress from the checklist.
4. Rules: minimal diffs, never print secrets, update `REPORT.md` per slice,
   park scope-creep in `TODO.md`.

## 8. Conventions

- Code idents: types `RustEz*`/`Ez*`, crates `rustez-*`, modules/fns `rustez_*`/`ez_*`,
  consts/env `RUSTEZ_*`/`EZ_*`. Docs may display `RustEZ`/`EZ`.
- Lints: `clippy::upper_case_acronyms = deny`, `unsafe_code = forbid`, no `allow`s.
- Only `rustez-compat` may mention `claw`/`openclaw` (verified by grep in review).
- Commits: concise imperative subject; push `main` only when asked (see CNP skill).
