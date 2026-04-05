# CONTEXT

## Technical

- Bridge: `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/workspace.rs` — `workspace_root_canon`, `resolve_under_workspace`, `read_workspace_text_file`, `list_workspace_entries`, `WorkspaceListEntry`.
- Tauri: `skilllite_read_workspace_file`, `skilllite_list_workspace_entries` registered in `lib.rs`.
- Frontend: `WorkspaceFileTree.tsx`, `WorkspaceIdeEditor.tsx`, `MainLayout.tsx` branch on `settings.ideLayout`.

## Compatibility

- Existing `skilllite_write_workspace_file` refactored to use shared resolvers; behavior unchanged for valid paths.
