# CONTEXT — Technical notes

## Data flow

React `ChatView` → `invoke("skilllite_chat_stream", { message, images?, ... })` → Tauri `chat_stream` builds JSON-RPC line with `params.images` → `skilllite agent-rpc` `handle_agent_chat` → `ChatSession::run_turn_with_media` → transcript append + `run_agent_loop(..., user_images)` → `ChatMessage::user_with_images` → `openai.rs` / `claude.rs` build API bodies.

## Types

- `skilllite_executor::transcript::TranscriptImage` — persisted `{ media_type, data_base64 }`.
- `skilllite_agent::types::UserImageAttachment` — re-export alias for the same shape.

## Compatibility

Older transcript lines without `images` deserialize with `images: None`.
