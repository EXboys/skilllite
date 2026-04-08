# CONTEXT

## Crate layout

- `skilllite-artifact/src/local_dir.rs` — moved from `skilllite-agent`.
- `skilllite-artifact/src/server.rs`, `client.rs`, `validation.rs` — from former `skilllite-artifact-http`.

## Agent dependency

```toml
skilllite-artifact = { path = "../skilllite-artifact", default-features = false, features = ["local"] }
```
