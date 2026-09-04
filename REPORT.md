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

## 6. Build slice 5 (2026-09-04, done) — e2e per functionality + repo

- `tests/`: `e2e_health` (1: /healthz ok+version), `e2e_config` (1: defaults + LIVESECRET→`***` masking + port reflect),
  `e2e_wizards` (2: 7 impls × auth(env_hint)+config steps; telegram unsupported),
  `e2e_cli` (3: status port, doctor empty/discord, invalid JSON fails),
  `e2e_traits` (3: sub-% math, SecretInput/AllowFrom round-trip, dummy impls run).
- `just e2e` → `cargo test --test 'e2e_*'`: 10 passed. Full `cargo test --workspace`: all green (13 new).
- Repo: `https://github.com/mars-crater/rustez` (public, main, Cargo.lock committed). Remote `origin` set.
- Run: `just e2e|just smoke|just lint`.

## 7. Build slice 6 (2026-09-04, done) — console bootstrap + OpenAI wizard + docs handoff

- `rustez onboard [--provider openai] [--token] [--model] [--docs docs/SETUP.md] [--non-interactive] [--skip-test] [--store-literal]`:
  welcome banner → token/model prompts (defaults offered; non-interactive uses flags+defaults) →
  live `ping_openai` test (chat/completions `ping`, max_tokens 8; `--skip-test` leaves item unchecked) →
  writes `rustez.json` (env-ref `{source:env,provider:default,id:EZ_OPENAI_SUB_TOKEN}` by default, literal only with flag + warning) +
  writes `docs/SETUP.md` (front-matter provider/model/config/status + checklist + Agent section).
- `EzAgentBootstrap{provider,model,docs}` + `prompt()` points the setup agent at the handoff doc
  ("continue with the next unchecked wizard item and update the checklist… never print secrets");
  resume order: Discord → usage → Qdrant → Proton Pass → email.
- `gateway` with no providers now prints the welcome hint; `doctor` reports `setup d/t (docs/SETUP.md)` when present.
- New dep (justified by test-it-out): `reqwest` rustls-minimal in `rustez-agent` only; token never logged.
- Verify: `fmt` ok, `clippy -D warnings` ok, `test` ok (32 suites incl. 3 bootstrap unit + 2 new e2e CLI onboard);
  manual smoke: onboard → SETUP.md with 6-item checklist + agent pointer, doctor shows `openai:true`.
- Run: `just onboard -- --token $T --model gpt-5.6-sol --non-interactive --skip-test`.

## 8. Build slice 7 (2026-09-04, done) — agent-readable docs

- `docs/AGENT.md` (committed): shared manual for anyone's agent — repo map, just commands + gates,
  trimmed config reference, API table, wizard table with env hints, resume protocol, conventions.
- Runtime `docs/SETUP.md` stays per-machine (gitignored) and now opens with
  "New here? Read docs/AGENT.md first". `README.md` points agents at it too.
- Verify: `fmt` ok, `clippy -D warnings` ok, `test` ok (32 suites).

## 9. Correction (2026-09-04) — OpenAI is OAuth, not subscription token

- Earlier slices said "subscription token" (`subToken`/`EZ_OPENAI_SUB_TOKEN` in wizard, onboard, README, AGENT.md).
  Corrected to OAuth: wizard field `oauthToken` → `EZ_OPENAI_OAUTH_TOKEN`, onboard stores the OAuth env-ref,
  docs updated. `ping_openai` already uses Bearer auth so it works with OAuth access tokens unchanged.
- Pinned by new unit test `openai_auth_is_oauth`. History above left intact.

## 10. Build slice 8 (2026-09-04, done) — ChatGPT OAuth dance, no token to paste

- Researched upstream (`openclaw/openclaw:extensions/openai`) + Codex OAuth pattern:
  public client `app_EMoamEEZ73f0CkXaXp7hrann`, PKCE S256, `auth.openai.com` authorize/token,
  scope `openid profile email offline_access`, `localhost:1455` callback, device-code fallback,
  rotating refresh, chat via `chatgpt.com/backend-api/codex` (Bearer + `ChatGPT-Account-Id`).
- New `rustez-agent::oauth`: PKCE/state/authorize-URL builders, std-only one-shot `:1455`
  callback server (state-validated), device flow (usercode → poll → exchange), code/refresh
  exchange, `id_token` identity decode, `0600` store (`~/.rustez/auth/openai.json`,
  `RUSTEZ_AUTH_DIR` override), margin auto-refresh, SSE `output_text.delta` chat.
- CLI: `auth login [--device|--paste|--no-open]` / `auth status` / `auth logout`;
  `onboard --provider openai` runs the dance (+`--device/--paste/--print-url/--paste-code+--verifier`),
  then a live codex ping, then `rustez.json` (no secret inside) + `docs/SETUP.md`.
- Wizard: openai steps describe the dance, zero required secret fields (pinned by tests).
- Secrets hygiene: `EzAuthProfile` has redacted `Debug`; error paths redact token material.
- New deps (justified): `base64`, `rand`, `sha2` in `rustez-agent` only.
- Verify: `fmt` ok, `clippy -D warnings` ok, `test` ok (incl. 6 oauth unit: RFC7636 vector,
  dance params, paste parse, crafted JWT, SSE, 0600 roundtrip; e2e: print-url JSON,
  non-interactive rejection, clean auth status, bad-provider login).
- Manual smoke: `--print-url` emits the exact upstream-shaped authorize URL; `auth status`
  clean when logged out. Real browser dance needs a human — not yet run end to end.
- Run: `just auth login` (or `--device` headless).

## 11. Build slice 9 (2026-09-04, done) — onboard TUI for the dance

- `src/tui.rs` (ratatui 0.29 + crossterm 0.28, binary-only): welcome → provider
  (openai enabled, others greyed TODO) → method [browser|device|paste] → model →
  paste-code (paste method only, shows its authorize URL, validates state) →
  confirm (summary + ping toggle) → live dance with spinner/log → done/failed.
- Shared core: prompt CLI refactored into `run_flow(&FlowParams, logger)` used by both
  frontends; TUI runs it on a worker thread via mpsc (log lines + finished event).
- `onboard` launches the TUI when interactive on a TTY, else falls back to prompts;
  `--non-interactive/--device/--paste/--print-url` paths unchanged (e2e untouched).
- Verify: 7 TUI unit tests (walk, defaults, method cycle, paste state validation,
  ping toggle, finished routing, log cap, Ctrl-C); pty-driven walk renders every
  screen through Confirm; `fmt`/`clippy -D warnings` clean, full workspace green.
- Run: `just onboard` (TTY) — real browser dance still needs a human at Confirm.

## 12. Fix (2026-09-04) — localhost callback robustness + recovery guidance

- Callback server now binds **both** `127.0.0.1:1455` and `[::1]:1455` (best-effort):
  browsers disagree on what `localhost` means, and the wrong half showed as
  "site can't be reached".
- New `preflight()`: any HTTP status from `auth.openai.com` counts as reachable;
  transport/TLS/DNS failures abort fast with a network/VPN/DNS hint instead of a
  confusing browser page. Wired into browser + device dance starts.
- Failure paths now spell out the recovery: approval succeeded but localhost
  unreachable → copy the address-bar URL (`?code=…` survives) → rerun with
  `--paste`; headless → `--device`. Same hint on the TUI Failed screen.
- Verify: `fmt`/`clippy -D warnings` clean, full workspace green.

## 13. Fix (2026-09-04) — whole-URL paste finishes the same dance

- `parse_pasted_code` strips surrounding quotes/whitespace: the whole address-bar
  URL pastes verbatim (quoted or not).
- Browser dance persists its verifier/state (`auth/.pending-openai.json`, `0600`):
  `--paste-code '<whole-url>'` resumes it with state-match validation and a 30-min
  freshness cap — approval survives a dead callback wait, no re-approval needed.
  Cleared on successful login. Explicit `--verifier` (scripted `--print-url`) unchanged.
- Also fixed a latent flake: env-mutating store tests now serialize on a mutex.
- Verify: `fmt`/`clippy -D warnings` clean, workspace green incl. 2 new oauth tests
  (quoted-URL paste, pending roundtrip + perms + redacted Debug).

## 14. Next

- Still TODO (needs onboarding specs): JSON5/`$include`/exec secrets, WS/RPC+pairing, Discord start/send, provider chat+`sub_usage` poll, full wizard fields+test/apply, focused-node UI components.
- Run: `EZ_CONFIG_PATH=./rustez.json cargo run -p rustez -- doctor|status|gateway`.
