# CONTEXT

## Technical

- Historical: first landed as `crates/skilllite-artifact-http`; merged into `crates/skilllite-artifact` in TASK-2026-027. Features are now `local`, `server`, `client` on `skilllite-artifact`.
- `ArtifactHttpState` holds `Arc<dyn ArtifactStore>` and optional bearer string; `artifact_router` applies auth middleware then handlers.
- Query parameter `key` carries the logical artifact key (may include `/`), avoiding ambiguous multi-segment path templates in OpenAPI.
- `HttpArtifactStore::try_new(base_url, bearer)` builds URLs as `{base}/v1/runs/{run_id}/artifacts?key=...`.

## Boundaries

- Depends only on `skilllite-core`; does not pull `skilllite-agent`.

## Follow-up

- Optional: `skilllite` CLI or agent feature to bind a listener; Python SDK thin HTTP wrapper.
