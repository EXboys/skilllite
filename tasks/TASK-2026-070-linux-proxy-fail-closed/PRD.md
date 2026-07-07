# PRD

## Background

SkillLite supports allowlisted network access by starting a local proxy for
`ProxyFiltered` policies. On Linux, bwrap/firejail currently keep outbound
sockets available, so skill code can bypass proxy environment variables and
connect directly. That converts an allowlisted policy into unrestricted egress
and violates fail-closed sandbox expectations.

## Objective

Linux sandbox execution must never run with unrestricted networking when a skill
requested domain-filtered egress. Unsupported proxy-filtered enforcement must
produce a clear execution error before the skill process starts.

## Functional Requirements

- FR-1: For `ProxyFiltered` policies, Linux bwrap/firejail execution is rejected as unsupported rather than downgraded to direct networking.
- FR-2: For `BlockAll`, Linux sandboxing continues to deny all network access.
- FR-3: For wildcard `AllowAll`, Linux sandboxing continues to allow direct network access without proxy.

## Non-Functional Requirements

- Security: Filtered network policy must fail closed if enforcement cannot be applied.
- Performance: No additional runtime overhead beyond a simple policy check.
- Compatibility: Explicit wildcard direct egress remains unchanged; filtered egress becomes stricter only on proxy setup failure.

## Constraints

- Technical: Keep the fix local to sandbox policy enforcement and avoid broad proxy refactors.
- Timeline: N/A for autonomous execution; scope is a focused security correction.

## Success Metrics

- Metric: Linux filtered-network-without-proxy decision.
- Baseline: Sandbox command can continue with direct/shared network.
- Target: Sandbox returns a validation error before command execution.

## Rollout

- Rollout plan: Ship as a patch-level security fix with regression tests.
- Rollback plan: Revert the small policy-check change if unexpected breakage appears; no data migration is involved.
