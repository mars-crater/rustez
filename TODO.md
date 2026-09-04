# RustEZ TODO — scope-creep parking lot

> V1 stays lightweight, blazing fast, intuitive, simple.
> Traits ship in v1. Concrete heavy impls (Qdrant, Proton Pass, etc.) are wired in **onboarding wizards**, not hardcoded now.

## Channels (`trait EzChannel` done, impls pending)
- [ ] telegram (long-poll first, webhook later)
- [ ] slack, whatsapp, signal, matrix, imessage, google-chat
- [ ] rich: topics/threads, polls, buttons, reactions, streaming edits, joinIntro
- [ ] nodes: camera/screen/location/canvas/desktop.stream
- [x] discord (v1 only)

## Providers (`trait EzProvider` done)
- [x] openai (subscription auth) — v1
- [x] opencode-go (API token) — v1
- [x] chutes (API token) — v1
- [ ] anthropic-messages, google, mistral, ollama, lmstudio, vllm, sglang, voyage, bedrock, deepinfra
- [ ] key rotation on 429/quota, cost catalog

## Usage
- [x] subscription-limit % (`EzSubUsage`, multi-limit, cached poll) — v1
- [ ] token spendings ledger, per-model costs, budgets/alerts, CSV export
- [ ] live quota APIs for opencode-go/chutes

## Memory (`trait EzMemory` done)
- [ ] Qdrant impl wiring (onboarding wizard: URL/collection/embed model + probe test)
- [ ] dreaming sweep, Honcho/LanceDB/wiki, file-journal promotion

## Voice (`trait EzTts/EzStt` done)
- [ ] local-CLI TTS + whisper-file STT wiring (onboarding)
- [ ] cloud voices, streaming audio

## Images (`trait EzImageGen` done)
- [ ] file-out impl wiring (onboarding)
- [ ] editing, continuity refs, provider gen (xAI etc.)

## Secrets (`trait EzSecretStore` done, BYO)
- [ ] Proton Pass via automizer token (onboarding wizard, redacted probe test)
- [ ] 1Password / Vault / env / file backends

## Email (`trait EzEmail` done)
- [ ] SMTP/IMAP impl wiring (onboarding, allowlist + approval gate)
- [ ] Gmail/Outlook OAuth, webhooks

## Gateway / UI
- [ ] Control UI full parity, Tailscale/funnel, mTLS, fleet/teams, Canvas/a2ui
- [ ] Focused-node editor: `ez-outline`, `ez-node-editor`, `ez-inspector`, `ez-wizard` (build after onboarding specs)
- [ ] `mcp serve`, browser tool, sandbox/docker
- [ ] EzHub registry, skill workshop, hooks SDK
