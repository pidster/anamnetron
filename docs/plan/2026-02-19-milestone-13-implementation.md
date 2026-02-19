# Milestone 13: Snapshot Diffing + Git Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable comparing two snapshots to see what changed (added/removed/changed nodes and edges), and auto-detect git HEAD when analyzing.

**Architecture:** New `diff` module in svt-core computes a `SnapshotDiff` by matching nodes on canonical path and edges on (source_path, target_path, kind). CLI gains `svt diff --from V1 --to V2`. Server gains `GET /api/diff?from=V1&to=V2`. Git auto-detection shells out to `git rev-parse HEAD` in the CLI when `--commit-ref` is not provided.

**Tech Stack:** Rust, serde (for JSON serialization of diff), `std::process::Command` (for git).

---

### Task 1: Core diff engine — SnapshotDiff types and diff_snapshots function

### Task 2: Add comprehensive tests including property-based diff symmetry

### Task 3: CLI `svt diff` command

### Task 4: Server `GET /api/diff` endpoint

### Task 5: Git auto-detection in CLI analyze command

### Task 6: Full verification, PROGRESS.md update, commit
