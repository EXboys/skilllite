# PRD

## Summary

Harden bash-tool command validation so backgrounding and I/O redirection cannot bypass the existing chain-operator gate before unsandboxed `sh -c` execution.

## Why

Bash-tool skills are an intentional exception path: after Rust-side validation they run on the host shell without bubblewrap. The validator is therefore the primary injection boundary. Omitting bare `&` / `>` / `<` leaves a concrete host command-execution and arbitrary-file-write hole.

## Requirements

1. Reject commands containing bare `&` (background / and-redirect forms).
2. Reject commands containing `>` or `<` (stdout/stdin/append redirects).
3. Keep existing protections for `;`, `&&`, `||`, `|`, backticks, `$()`, `${}`, newlines, and `>(`.
4. Document the operator set in EN/ZH architecture security notes.

## Non-goals

- Quote-aware or AST-based shell parsing in this change.
- Moving bash-tool execution into the sandbox.
