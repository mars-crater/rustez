# RustEZ REPORT — ledger of work + decisions

Date: 2026-09-04 (UTC) · Repo: `~/src/rustEZ` · Source: `github.com/openclaw/openclaw`
Guiding principle (decision maker): **lightweight and blazing fast as possible, intuitive and simple.**

## 0. Trim decision (2026-09-04, build mode)

Per request, schemas trimmed to tiny scope — OpenClaw-like shapes, minimal fields only:
gateway{port/mode/bind/auth} + discord{enabled/token/dmPolicy/allowFrom/requireMention/historyLimit}
+ providers{openai/opencode-go/chutes: baseUrl/apiKey/api/timeout/models[{id}]} + memory{provider/model/remote}
+ mcp{servers[{enabled/command/args/url/transport}]} + SecretInput/SecretRef + AllowFrom + passthrough `extra`.
Full upstream shapes (telegram, guilds/voice details, catalog/cost, FTS/vector opts, OAuth, tailscale, reload) deferred to TODO/onboarding.
Verified after trim: `fmt` ok, `clippy -D warnings` ok, `test` ok (0 tests).

## 1. Work fulfilled (this session)

- [x] Toolchain: `rustup default stable` → `rustc 1.98.1`, `cargo 1.98.1` (was unconfigured).
- [x] Scaffold: Cargo workspace `rustez 0.1.0` + binary `src/main.rs` (`status|gateway|doctor` stubs).
- [x] 13 trait-only crates (no heavy deps, no Qdrant/Proton/mail/cloud SDKs):
  - `rustez-config` — `RustEzConfig/Gateway/Providers/Channels`, `rustez_load()` stub
  - `rustez-compat` — only place allowed to say `claw/openclaw`; `from_openclaw_key`, `ez_env` (`EZ_*`→`RUSTEZ_*`→`OPENCLAW_*` fallback), `passthrough`
  - `rustez-gateway` — `EzGateway`, `EzHealth`, `RUSTEZ_DEFAULT_PORT=18790` (Node keeps 18789)
  - `rustez-agent` — `EzProvider {id/chat/sub_usage}`, `EzChatReq/Resp`, `EzWizardStep`
  - `rustez-channels` — `EzChannel {id/start/send}`, `EzOutMsg`, `EzChanCtx`
  - `rustez-memory` — `EzMemory {kind/put/search/get}`, Qdrant-minded, no client yet
  - `rustez-voice` — `EzTts::speak`, `EzStt::transcribe`
  - `rustez-images` — `EzImageGen::generate`
  - `rustez-secrets` — `EzSecretStore::resolve` (BYO; Proton Pass via onboarding)
  - `rustez-email` — `EzEmail {send/search}` (allowlisted, approval-gated)
  - `rustez-usage` — `EzSubUsage {limits: Vec<EzSubLimit>}`, `pct()` helper
  - `rustez-mcp` — `EzMcpServer` stub · `rustez-cli` — `RUSTEZ_CLI_VERSION`
- [x] `TODO.md` (root): scope-creep parking lot for all non-v1 channels/providers/backends.
- [x] UI stub: `ui/src/ez-config/ez-config-page.ts` placeholder for focused-node grid.
- [x] Verification (evidence):
  - `cargo check --workspace` → ok (5.6s)
  - `cargo fmt --check` → ok (after 1 autofix)
  - `cargo clippy --all-targets -- -D warnings` → ok
  - `cargo test --workspace` → ok (0 tests, 0 failed)

## 2. Decisions made (with rationale)

1. **No heavy impls now** — Qdrant, Proton Pass automizer, SMTP, TTS/STT, image gen are traits only; concrete wiring lives in **onboarding wizards** (`GET /api/wizards/:impl`, `POST .../test`, `POST /api/config/apply`). Keeps v1 light/fast/simple.
2. **Naming (lint-clean)**: `Claw→Ez/ez`, `OpenClaw→RustEz*` code (`RustEZ` docs only), crates `rustez-*`, const/env `RUSTEZ_*/EZ_*`. `clippy::upper_case_acronyms="deny"`, `unsafe="forbid"`, no `allow`s.
3. **Channels**: global `EzChannel` trait now, **Discord-only** v1; rest in TODO.
4. **Providers**: `openai` sub-only + `opencode-go`/`chutes` token-only; others/rotation/costs in TODO.
5. **Usage**: v1 = subscription-limit % (`EzSubLimit{pct}`, multi-limit, cached poll). Token spendings ledger in TODO.
6. **UI**: focused-node, not a clone — `280px outline | 1fr center (only scroller, 100dvh) | 340px inspector`; schema-registry rendering, form↔raw sync, per-impl wizards. Keeps JSON pages intuitive.
7. **Compat**: unknown keys pass through; `rustez.json` + `~/.rustez/` canonical, `openclaw.json`/`OPENCLAW_*`/wire strings accepted only via `rustez-compat`.

## 3. Build slice 2 (2026-09-04, done — still lightweight)

- `rustez_load` real (JSON only; missing/empty → default; JSON5/`$include`/exec in TODO) + 2 unit tests pass.
- Gateway serves on `127.0.0.1:{port}` (default 18790): `GET /healthz`, `GET /api/config` (secret literals masked to `***`), `GET /api/wizards/:key` stubs (`supported=true` for discord/openai/opencode-go/chutes/qdrant/proton-pass/email).
- CLI real: `status` (port+path), `doctor` (mode/port/discord/providers), `gateway` (bind+serve).
- Smoke (evidence): `doctor` ok empty + discord-true; `curl /healthz` → `{"ok":true,"version":"0.1.0"}`; `/api/config` masks `LIVESECRET`→`***`; `/api/wizards/discord` supported:true, `/api/wizards/telegram` supported:false.
- Verify: `fmt` ok, `clippy -D warnings` ok, `test` ok (2 passed).
- Still TODO (needs onboarding specs): JSON5/`$include`/exec secrets, WS/RPC+pairing, Discord start/send, provider chat+sub_usage poll, full wizard fields+test/apply, focused-node UI components.

## 4. Build slice 3 (2026-09-04, done) — wizards carry auth+config

- `EzWizard{key,title,steps,supported}` + `EzWizardStep{id,title,help,fields}` + `EzField{key,label,kind,required,auth,placeholder?,env_hint?}` (`kind`: text|secret|url|select|bool|number).
- Per impl 2 steps (`auth` then `config`), mirroring trimmed `rustez-config` shapes:
  discord{token→EZ_DISCORD_TOKEN | dmPolicy/allowFrom/requireMention/historyLimit},
  openai{subToken→EZ_OPENAI_SUB_TOKEN | model/pollSecs},
  opencode-go{apiToken→EZ_OPENCODEGO_TOKEN | baseUrl/models},
  chutes{apiToken→EZ_CHUTES_TOKEN | baseUrl/models},
  qdrant{apiKey?→RUSTEZ_QDRANT_KEY | url/collection/embedModel},
  proton-pass{automizerToken→EZ_PROTONPASS_AUTOMIZER_TOKEN | storeId},
  email{smtpUser/smtpPass | host/port/allowTo}. Others → unsupported stub.
- Smoke: `/api/wizards/discord` returns auth token field + config fields; `/api/wizards/proton-pass` returns automizerToken auth. Unit test `wizards_carry_auth_and_config` passes (7 impls × auth+config).
- Verify: `fmt` ok, `clippy -D warnings` ok, `test` ok (3 passed: 2 config + 1 gateway).

## 5. Build slice 4 (2026-09-04, done) — justfile runner

- `justfile` (just 1.21.0, `apt`): `default: check`; recipes `build/release/check/fmt/lint/test/run/doctor/gateway/wiz/smoke/install/clean` — thin cargo wrappers, keeps RustEZ light.
- Verify via just (evidence): `just check` ok, `just test` ok (25×0-pass + 1×1-pass + 1×2-pass suites), `just lint` ok, `just smoke` → `SMOKE_OK` (doctor ok, health `{"ok":true}`, discord wizard supported:true).
- Run: `just doctor|just gateway|just wiz proton-pass|just smoke`.

## 6. Next (needs onboarding values to go live)

- Still TODO (needs onboarding specs): JSON5/`$include`/exec secrets, WS/RPC+pairing, Discord start/send, provider chat+`sub_usage` poll, full wizard fields+test/apply, focused-node UI components.
- Run: `EZ_CONFIG_PATH=./rustez.json cargo run -p rustez -- doctor|status|gateway`.
