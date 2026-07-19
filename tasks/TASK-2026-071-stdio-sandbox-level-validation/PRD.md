# PRD

## Background

Stdio JSON-RPC accepts numeric JSON values as `u64`, but the execution request
parser currently converts `sandbox_level` with `as u8`. Rust truncates this cast,
so an invalid value such as `257` becomes the documented level `1` and disables
the sandbox. The MCP path already rejects values outside 1 through 3.

## Objective

Make stdio skill execution fail closed when `sandbox_level` is malformed while
preserving valid levels and omitted-field behavior.

## Functional Requirements

- FR-1: `run` and `exec` requests must accept only JSON integers 1, 2, or 3.
- FR-2: Missing `sandbox_level` must continue to defer to environment/default
  selection.
- FR-3: Invalid types and out-of-range integers must produce a parameter error
  before any skill execution begins.

## Non-Functional Requirements

- Security: No integer truncation or permissive fallback may weaken isolation.
- Performance: Validation must be constant-time with no new allocation on the
  successful numeric path.
- Compatibility: Valid clients and requests omitting the field remain unchanged.

## Constraints

- Technical: Keep the fix inside stdio parameter parsing and use the crate error
  type; add no dependency.
- Timeline: N/A; deliver as one minimal security fix.

## Success Metrics

- Metric: Parser handling of sandbox levels across `run` and `exec`.
- Baseline: `257` is accepted and converted to level `1`.
- Target: `257`, zero, values above three, fractional values, and strings are
  rejected; 1 through 3 and omission remain accepted.

## Rollout

- Rollout plan: Ship with the next binary and Python SDK bundled-binary release.
- Rollback plan: Revert the parser helper and its call sites if valid requests
  regress; do not restore the truncating cast as a compatibility workaround.
