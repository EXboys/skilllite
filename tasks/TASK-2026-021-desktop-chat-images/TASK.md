# TASK Card

## Metadata

- Task ID: `TASK-2026-021`
- Title: Desktop chat: user image attachments (vision)
- Status: `done`
- Priority: `P2`
- Owner: `maintainer`
- Contributors:
- Created: `2026-04-04`
- Target milestone:

## Problem

Desktop users could not attach images to chat turns; the agent stack only sent plain text to LLM APIs, so vision-capable models could not see screenshots or photos.

## Scope

- In scope: Desktop UI (pick images, preview, send), Tauri bridge + `agent_chat` params, executor transcript persistence, agent `ChatMessage` + OpenAI/Claude multimodal serialization, docs (assistant README + entrypoints EN/ZH).
- Out of scope: CLI image upload, automatic image compression beyond client-side file size cap, non-vision model detection beyond MiniMax guard.

## Acceptance Criteria

- [x] User can attach up to 6 images (PNG/JPEG/WebP/GIF, ≤5MB each) and send with optional text; empty text allowed when images present.
- [x] `agent_chat` accepts optional `images[]` with `media_type` + `data_base64`; transcript stores images on user `message` rows.
- [x] OpenAI-compatible and Claude API requests include vision parts when history or current turn has images.
- [x] Reloading session shows image previews for user rows that have `images` in transcript.
- [x] MiniMax Coding Plan path rejects image attachments with a clear error.

## Risks

- Risk: Large base64 in jsonl transcripts
  - Impact: Disk and load cost
  - Mitigation: Caps on count and payload size at RPC; user-facing limits in UI

- Risk: Non-vision models return API errors
  - Impact: User confusion
  - Mitigation: Document requirement for vision models in assistant README

## Validation Plan

- Required tests: `cargo test -p skilllite-agent openai_attachment_tests`
- Commands run: `cargo clippy -p skilllite-agent -p skilllite-executor -- -D warnings`, `cargo check --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml`, `npm run build` in `crates/skilllite-assistant`
- Manual checks: Desktop `tauri dev` — attach image, send with gpt-4o or Claude, reload session

## Regression Scope

- Areas likely affected: `agent_chat` RPC, transcript format, assistant Tauri command signature, `ChatMessage` struct, evolution adapter mapping
- Explicit non-goals: Changing CLI chat UX

## Links

- Related docs: `crates/skilllite-assistant/README.md`, `docs/zh/ENTRYPOINTS-AND-DOMAINS.md`, `docs/en/ENTRYPOINTS-AND-DOMAINS.md`
