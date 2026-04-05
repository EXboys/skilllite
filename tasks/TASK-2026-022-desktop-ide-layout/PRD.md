# PRD

## Summary

Add an optional **IDE layout** to the desktop assistant: **file tree | editor | chat**, similar to Cursor, while keeping the classic **sessions | chat | status** layout when disabled.

## Decisions

- Toggle: header **IDE** button + **Settings → Workspace** switch; persisted as `ideLayout` in the settings store.
- Left column tabs: **Files** (tree) and **Sessions** (existing `SessionSidebar`).
- Listing: new `skilllite_list_workspace_entries` with skip rules for `node_modules`, `target`, etc.; max depth/entry limits.
- Reading: `skilllite_read_workspace_file` shares path rules with `skilllite_write_workspace_file` (including sensitive path blocks).
- IDE mode hides the right **StatusPanel**; Life Pulse badge remains in the header.

## Non-goals

Resizable splitters, LSP, and binary file editing.
