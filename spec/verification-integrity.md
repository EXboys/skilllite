# Verification Integrity (Anti-Hallucination)

Scope: **all** tasks. This is the highest-priority spec — it overrides convenience trade-offs in every other spec.

## Core Principle

> A change is not done when the model says it is done.
> A change is done when independently verifiable evidence proves it is done.

AI-assisted development introduces a systemic risk: the model may **claim** work is complete, tests pass, or behavior is correct — without that being true. Every workflow gate must defend against this.

## Must

### Anti-Hallucination (claim ≠ fact)

- Every behavioral claim ("fixed", "added", "passing") MUST have a corresponding verification command output or test result as evidence.
- Do NOT trust a model's assertion that "all tests pass" without running `cargo test` (or equivalent) and inspecting the output.
- After modifying code, re-read the actual file content to confirm the edit landed correctly. Do not assume the edit succeeded based on intent alone.
- When a model says it "removed" duplicate logic, verify both the removal site AND the replacement site independently.
- Cross-reference: if the model says it updated N files, verify all N files were actually modified.

### Anti-False-Positive (test ≠ proof)

- A test MUST fail when the feature it guards is broken. If you cannot demonstrate this (mentally or actually), the test is suspect.
- Tests that assert on error message substrings MUST match the actual error path's output — not a plausible-sounding string the model invented.
- Compatibility/legacy bypass layers (e.g. `accepts_legacy_field_shape`) MUST have paired tests: one proving the bypass works, one proving the non-bypass path still validates.
- When a test was failing and a "fix" makes it pass, verify the fix addresses the root cause — not that it silences the assertion (e.g. by loosening the check or catching the error).
- Do NOT write tests that pass vacuously (e.g. asserting `result.is_ok()` on an operation that never actually runs the code under test).
- For error summarization / truncation logic, verify non-ASCII inputs (CJK/emoji) do not panic and preserve UTF-8 boundaries.

### Anti-Drift (code ≠ schema ≠ docs)

- When validation logic (whitelist, enum, range) is added in code, verify that the corresponding schema definition and documentation are consistent.
- When a tool definition declares `required: ["x"]`, verify that the runtime validator actually enforces it — not just that a test claims it does.
- When metadata (e.g. `ToolExecutionProfile`) is derived automatically, verify the derivation matches the tool's actual behavior (e.g. a state-mutating tool must not be marked `is_read_only`).

## Must Not

- Do NOT mark a task complete based solely on "the model said it worked."
- Do NOT skip re-running the full test suite after late-stage fixes (especially compatibility patches).
- Do NOT accept a model's self-review ("I checked and it looks correct") as a substitute for mechanical verification.
- Do NOT allow a test to assert on a return value that the test itself constructs (circular validation).

## Verification Checklist (every task)

- [ ] Were all behavioral claims verified by running actual commands (`cargo test`, `cargo clippy`, `cargo fmt --check`)?
- [ ] Were modified files re-read to confirm edits landed as intended?
- [ ] Do tests fail when the guarded behavior is removed? (falsifiability)
- [ ] Are error-message assertions sourced from actual error paths, not invented strings?
- [ ] Is there consistency between code validation logic, schema definitions, and documentation?
- [ ] For any bypass/compatibility layer: do paired tests exist proving both the bypass and the guarded path?
- [ ] Were task artifacts (`TASK.md`, `STATUS.md`, `board.md`) re-read after update to confirm they reflect the final state?
- [ ] Were error/fallback paths (not just happy path) exercised and confirmed panic-free with realistic inputs?

## Known Failure Patterns (from real incidents)

1. **Whitelist ≠ Alias coverage**: Code validated against a whitelist `["a", "b", "c"]` while downstream handlers also accepted aliases `["a2", "b2"]`. The whitelist blocked valid inputs. *(Detected in TASK-2026-004)*
2. **Profile metadata drift**: `ToolExecutionProfile` was auto-derived from capabilities, but tools with empty capabilities (planning control tools) were incorrectly marked `is_read_only=true` despite mutating state. *(Detected in TASK-2026-004)*
3. **Error message mismatch**: Tests pre-loaded `record_failure` with an error string that didn't match what the actual validator produced after refactoring. The test still passed because the prefix-match window was wide enough — but the intent was fragile. *(Near-miss in TASK-2026-004)*
4. **Compound TODO skip**: A TODO item bundled multiple file updates ("更新 STATUS.md, TASK.md, board.md") into one entry. The model updated the first two, marked the TODO complete, and skipped `board.md`. The completion gate said "board status is updated" but was not mechanically verified (no re-read). *(Detected in TASK-2026-002)*
