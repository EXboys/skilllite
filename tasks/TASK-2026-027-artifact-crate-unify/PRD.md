# PRD

## Goal

One crate owns all `ArtifactStore` implementations shipped in-tree (local directory + optional HTTP), while `ArtifactStore` remains in `skilllite-core`.

## Behavior

- Agent continues to use `LocalDirArtifactStore` under chat `data_root`; dependency graph slims via feature flags.
