# PRD — Desktop chat image attachments

## Goal

Enable multimodal user turns from the SkillLite Assistant desktop app so vision-capable LLMs can reason over attached images.

## Requirements

1. **Desktop**: File picker for common image types; thumbnails with remove; send with text or image-only.
2. **Protocol**: Extend `agent_chat` params with optional `images` array; validate MIME, size, and count server-side in the agent RPC handler.
3. **Persistence**: Store images on `TranscriptEntry::Message` for user rows; load back into UI as data URLs.
4. **LLM**: Map user messages with images to OpenAI `content` array and Claude image blocks.

## Non-requirements

- Video, PDF, or arbitrary file attachments in this task.
- URL-based images (only base64 from desktop).
