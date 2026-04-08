# CONTEXT

## Components

- `skilllite artifact-serve`: `--dir`, `--bind`, optional `--token`; prints `SKILLLITE_ARTIFACT_HTTP_ADDR=...` on stdout; **bind** requires `SKILLLITE_ARTIFACT_SERVE_ALLOW=1`.
- Python tests resolve the `skilllite` executable via `SKILLLITE_ARTIFACT_HTTP_SERVE` or `../../target/debug|release/skilllite`, then spawn `artifact-serve` with allow-env set.

## Test naming

`test_scenario_*` docstrings describe user-visible flows (model output, multi-file session, run isolation, missing key, bearer).
