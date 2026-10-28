# PRD

## Background

Subscription sandbox runs pin egress to `https_proxy` on Linux by attaching an nftables output filter to the run's cgroup. That filter is the only network boundary: the bwrap profile does not use a network namespace. The rule shipped on 2026-10-28 names the cgroup by its leaf directory. nftables treats the match string as a path under `/sys/fs/cgroup`, so the rule cannot be installed for a nested cgroup and the run refuses to start.

## Objective

A Linux run with `https_proxy` set installs an nftables allowlist for that proxy and drops other TCP and UDP from the run cgroup. If the rule cannot be installed, the run still does not start.

## Functional Requirements

- FR-1: The nftables match path is the cgroup directory relative to `/sys/fs/cgroup`.
- FR-2: `socket cgroupv2 level` equals the number of components in that path.
- FR-3: Paths outside the cgroup root, or paths that contain `"`, `\`, newlines, `{`, or `}`, do not produce a script.
- FR-4: The output chain policy stays `accept` so traffic from other host processes is unchanged.

## Non-Functional Requirements

- Security: A non-matching level must not be emitted. That combination installs on nftables 1.0.9 and then does not filter, because the chain policy is accept.
- Performance: No extra process or mount.
- Compatibility: Runs that omit `https_proxy` stay unfiltered. macOS Seatbelt is unchanged.

## Constraints

- Technical: Enforcement still depends on nftables and a writable child cgroup. Those failures remain fail-closed.
- Timeline: Minimal change to the script builder and its caller.

## Success Metrics

- Metric: Nested cgroup script contains the full relative path and the matching level.
- Baseline: Script contained only `skilllite-<run>` and nftables 1.0.9 rejected it.

## Non-Goals

- Replacing nftables with a network namespace.
- Narrowing macOS file reads.
