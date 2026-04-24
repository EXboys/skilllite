# PRD

## Background

SkillLite now has two separate HTTP-serving stories that are both valid in isolation:

- `skilllite channel serve` for inbound webhook MVP.
- `skilllite artifact-serve` for run-scoped artifact HTTP.

That split was a deliberate low-risk choice for MVP, but it is not the most natural shape for future multi-platform deployments. As the product moves toward more cross-platform interaction, operators need a single, durable host process where HTTP surfaces can be attached without forcing the domain crates themselves to merge.

This task matters now because the repository has already learned that broad “shared services” extraction can be premature. A gateway bootstrap is a narrower, more concrete move: unify HTTP hosting first, keep domain crates independent, and leave deeper routing/session work for later tasks.

## Objective

Introduce a new `skilllite gateway serve` command that becomes the preferred unified HTTP host for inbound webhook and optional artifact routes, while keeping current standalone commands available for compatibility.

Do so without weakening security defaults or requiring Assistant / Python SDK / MCP migrations in the same change.

## Functional Requirements

- FR-1: Users can start a new unified host with `skilllite gateway serve`.
- FR-2: The gateway host exposes `GET /health`.
- FR-3: The gateway host exposes `POST /webhook/inbound` with the same MVP webhook behavior already available in `skilllite channel serve`.
- FR-4: The gateway host can optionally expose artifact HTTP routes when an artifact directory is provided.
- FR-5: Existing `skilllite channel serve` and `skilllite artifact-serve` commands continue to work in this phase.
- FR-6: Documentation clearly explains that gateway is the new unified host direction, while standalone channel/artifact serve commands remain supported compatibility entry points.

## Non-Functional Requirements

- Security:
  - Binding must remain fail-closed behind an explicit allow gate.
  - Non-loopback binding must require an auth token unless an explicit insecure override is set.
  - Existing artifact/channel auth expectations must not silently relax when hosted by the gateway.
- Performance:
  - The gateway host should remain lightweight and reuse current Axum-based serving patterns.
  - No new long-running background polling or heavyweight orchestration should be introduced in this phase.
- Compatibility:
  - Existing standalone CLI commands remain available.
  - Existing artifact HTTP API path shape must stay compatible when mounted under the gateway host.
  - Existing Assistant channel settings page may remain on the old command for now.

## Constraints

- Technical:
  - Respect the existing crate layering and avoid reviving the rolled-back `skilllite-services` abstraction.
  - Prefer additive integration using current domain crates (`skilllite-artifact`, current webhook logic) over deep migration.
  - Keep terminology precise: gateway is the host; channel/artifact remain domain capabilities.
- Timeline:
  - One mergeable task only: bootstrap the host and docs, do not bundle follow-up UI or SDK migrations.

## Success Metrics

- Metric: Number of long-running HTTP host entry points needed for the combined inbound webhook + artifact use case.
- Baseline: Two separate processes/commands are required today.
- Target: One new gateway host can serve both surfaces in a single process, while old commands remain available.
- Metric: Security regression count.
- Baseline: Existing commands are gated and fail-closed by default.
- Target: Zero default-security regressions in the new host.

## Rollout

- Rollout plan:
  - Ship `skilllite gateway serve` as an additive command.
  - Keep old standalone commands documented as compatibility paths.
  - Follow-up tasks may migrate Assistant/help text later once the host proves stable.
- Rollback plan:
  - If the new gateway host proves unstable, leave existing `channel serve` and `artifact-serve` as the primary documented paths and remove/disable the new command in a focused follow-up.
