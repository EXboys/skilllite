# PRD

## Background

`agent-sandbox` can start without `https_proxy`. Docs say that egress is not pinned. On Linux the process still could not resolve hostnames, because the bwrap plan never mounted the name-service files that skill `execute()` already mounts. Reproduced on 2026-10-30: Python `socket.getaddrinfo("example.com", 443)` inside that mount set returns `[Errno -3] Temporary failure in name resolution`. Adding the three files makes the same lookup succeed. A numeric address resolves either way, which is why a proxied run (proxy URL rewritten to an IP) did not show the bug.

## Objective

An unproxied Linux subscription run can resolve hostnames. A proxied run still does not mount name-service files and still does not get a DNS allow rule.

## Functional Requirements

- FR-1: When `https_proxy` is empty, the Linux bwrap plan read-only mounts `/etc/resolv.conf`, `/etc/nsswitch.conf`, and `/etc/hosts` if each file exists.
- FR-2: When `https_proxy` is set, those three paths are not mounted.
- FR-3: Missing files are skipped so bwrap is not asked to bind a path that is not there.

## Non-Functional Requirements

- Security: No new egress when a proxy is set. The nftables tcp/udp drop stays as it is. Name-service files are read-only and only appear on the unproxied path, whose egress is already unpinned.
- Performance: Three extra read-only binds on the unproxied path only.
- Compatibility: macOS Seatbelt already allows `file-read*`, so its profile does not change. CLI flags do not change.

## Constraints

- Technical: Same bwrap `--ro-bind` pattern as the skill sandbox. Do not add `--unshare-net`.
- Timeline: Bugfix only.

## Success Metrics

- Metric: Unproxied plan contains the three `--ro-bind` entries; proxied plan contains none; a real unproxied `run` can read the host files.
- Baseline: Unproxied plan contained none of the three paths, and `getaddrinfo` returned `-3`.

## Open Questions

- None. Proxied runs stay without DNS on purpose.
