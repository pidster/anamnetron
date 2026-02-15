# ADR-007: Snapshot Versioning with Monotonic Version Numbers

## Status

Accepted

## Context

The tool needs to track the graph's state over time for drift detection (comparing two analysis runs) and conformance (comparing design against analysis). The temporal model must support: querying the current state, comparing two points in time, and pruning old data.

## Decision

Use monotonic integer version numbers to identify snapshots. Nodes, edges, and constraints carry a `version` field. Each analysis run, design revision, or import creates a new snapshot with the next version number. Drift detection and conformance are queries across version-tagged data.

## Alternatives Considered

- **Timestamps** — ambiguous (timezones, clock drift, wall clock vs logical time). Don't express ordering as cleanly as integers.
- **Git-native (snapshots = commits)** — export graph to JSON, commit to git, diff with git. Simple, but can't query across time inside the tool. Raw JSON diffs are unreadable.
- **Event sourcing** — store `NodeAdded`, `EdgeAdded` events, derive state by replay. Most powerful, most complex. Deferred as a future option.
- **Soft deletes (created_at, removed_at)** — graph accumulates history, never shrinks. Every "current state" query must filter by `removed_at IS NULL`. Complexity tax on all queries.

## Consequences

- Simple, deterministic ordering — version 5 is always after version 4.
- Design and analysis snapshots share one version sequence — gives a unified timeline.
- Storage grows linearly with snapshots — mitigated by snapshot compaction (configurable retention policy).
- The compound key `(id, version)` means the same logical node appears once per snapshot.
- Drift detection is a straightforward Datalog query: "nodes in version B not in version A."
