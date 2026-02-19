# Milestone 9: CI Pipeline — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** GitHub Actions CI workflow with full-stack checks (Rust + WASM + web) and conformance dog-food gate.

**Architecture:** Single GitHub Actions workflow triggered on push/PR to `main`. Sequential steps in one job sharing build cache. Conformance check runs with `continue-on-error: true` and writes a Markdown summary to `$GITHUB_STEP_SUMMARY`.

**Tech Stack:** GitHub Actions, `dtolnay/rust-toolchain`, `Swatinem/rust-cache@v2`, `actions/setup-node@v4`, wasm-pack, cargo-audit

---

### Task 1: Create Audit Ignore Config

**Files:**
- Create: `.cargo/audit.toml`

CozoDB 0.7 has 3 unmaintained (but not vulnerable) transitive dependencies that cause `cargo audit` to fail. We need an ignore list so CI stays green.

**Step 1: Create `.cargo/audit.toml`**

```toml
[advisories]
ignore = [
    "RUSTSEC-2025-0056",  # adler: unmaintained, used by cozo transitive dep
    "RUSTSEC-2025-0141",  # bincode: unmaintained, used by cozo transitive dep
    "RUSTSEC-2025-0057",  # fxhash: unmaintained, used by cozo transitive dep
]
```

**Step 2: Verify audit passes locally**

Run: `cargo audit`
Expected: No warnings or errors (the 3 known advisories are suppressed).

**Step 3: Commit**

```bash
git add .cargo/audit.toml
git commit -m "chore: add cargo audit ignore list for cozo transitive deps"
```

---

### Task 2: Create CI Workflow — Rust Checks

**Files:**
- Create: `.github/workflows/ci.yml`

This task creates the workflow file with the Rust-only steps: checkout, toolchain, cache, fmt, clippy, test, audit. Web and WASM steps are added in subsequent tasks.

**Step 1: Create directory**

Run: `mkdir -p .github/workflows`

**Step 2: Create `.github/workflows/ci.yml`**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    name: Build & Test
    runs-on: ubuntu-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
          targets: wasm32-unknown-unknown

      - name: Cargo cache
        uses: Swatinem/rust-cache@v2

      - name: Format check
        run: cargo fmt --check

      - name: Clippy
        run: cargo clippy -- -D warnings

      - name: Test
        run: cargo test

      - name: Install cargo-audit
        run: cargo install cargo-audit --locked

      - name: Audit
        run: cargo audit
```

**Step 3: Verify Rust CI commands locally**

Run each command and confirm they all pass:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo audit
```

Expected: All four pass with exit code 0. Tests report 277 passing.

**Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add GitHub Actions workflow with Rust checks"
```

---

### Task 3: Add WASM Build Step

**Files:**
- Modify: `.github/workflows/ci.yml` (append after Audit step)

**Step 1: Add WASM build step to `.github/workflows/ci.yml`**

Insert after the `Audit` step:

```yaml
      - name: Install wasm-pack
        run: cargo install wasm-pack --locked

      - name: WASM build
        run: wasm-pack build crates/wasm --target web
```

**Step 2: Verify WASM build locally**

Run: `wasm-pack build crates/wasm --target web`
Expected: Build succeeds, producing `crates/wasm/pkg/` with `.wasm`, `.js`, and `.d.ts` files.

**Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add WASM build verification step"
```

---

### Task 4: Add Web Frontend Steps

**Files:**
- Modify: `.github/workflows/ci.yml` (append after WASM build step)

**Step 1: Add Node.js setup and web test steps to `.github/workflows/ci.yml`**

Insert after the `WASM build` step:

```yaml
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm
          cache-dependency-path: web/package-lock.json

      - name: Web install
        run: npm ci
        working-directory: web

      - name: Web test
        run: npm test
        working-directory: web
```

**Step 2: Verify web commands locally**

```bash
cd web && npm ci && npm test
```

Expected: `npm ci` installs cleanly, `npm test` runs vitest and reports 5 passing tests.

**Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add web frontend install and test steps"
```

---

### Task 5: Add Conformance Gate with Step Summary

**Files:**
- Modify: `.github/workflows/ci.yml` (append after Web test step)

This is the dog-food check: import the project's own design model, analyze the codebase, and compare. The conformance step uses `continue-on-error: true` so CI stays green even when architecture drifts during development. Results are written to `$GITHUB_STEP_SUMMARY` for visibility.

**Step 1: Add conformance steps to `.github/workflows/ci.yml`**

Insert after the `Web test` step:

```yaml
      - name: Conformance check
        id: conformance
        continue-on-error: true
        run: |
          cargo run --bin svt -- import design/architecture.yaml
          cargo run --bin svt -- analyze .
          cargo run --bin svt -- check --analysis --format json > conformance.json

      - name: Conformance summary
        if: always() && steps.conformance.outcome != 'skipped'
        run: |
          if [ -f conformance.json ]; then
            TOTAL=$(jq '.results | length' conformance.json)
            PASS=$(jq '[.results[] | select(.status == "Pass")] | length' conformance.json)
            FAIL=$(jq '[.results[] | select(.status == "Fail")] | length' conformance.json)
            WARN=$(jq '[.results[] | select(.status == "Warn")] | length' conformance.json)
            NOT_EVAL=$(jq '[.results[] | select(.status == "NotEvaluable")] | length' conformance.json)

            echo "## Conformance Report" >> "$GITHUB_STEP_SUMMARY"
            echo "" >> "$GITHUB_STEP_SUMMARY"
            echo "| Status | Count |" >> "$GITHUB_STEP_SUMMARY"
            echo "|--------|-------|" >> "$GITHUB_STEP_SUMMARY"
            echo "| Pass | $PASS |" >> "$GITHUB_STEP_SUMMARY"
            echo "| Fail | $FAIL |" >> "$GITHUB_STEP_SUMMARY"
            echo "| Warn | $WARN |" >> "$GITHUB_STEP_SUMMARY"
            echo "| Not Evaluable | $NOT_EVAL |" >> "$GITHUB_STEP_SUMMARY"
            echo "| **Total** | **$TOTAL** |" >> "$GITHUB_STEP_SUMMARY"

            if [ "$FAIL" -gt 0 ]; then
              echo "" >> "$GITHUB_STEP_SUMMARY"
              echo "### Failures" >> "$GITHUB_STEP_SUMMARY"
              echo "" >> "$GITHUB_STEP_SUMMARY"
              jq -r '.results[] | select(.status == "Fail") | "- **\(.constraint)**: \(.message)"' conformance.json >> "$GITHUB_STEP_SUMMARY"
            fi
          else
            echo "## Conformance Report" >> "$GITHUB_STEP_SUMMARY"
            echo "" >> "$GITHUB_STEP_SUMMARY"
            echo "Conformance check did not produce output." >> "$GITHUB_STEP_SUMMARY"
          fi
```

**Step 2: Verify conformance commands locally**

```bash
cargo run --bin svt -- import design/architecture.yaml
cargo run --bin svt -- analyze .
cargo run --bin svt -- check --analysis --format json > conformance.json
```

Expected: `conformance.json` is produced with a `results` array. Each result has `constraint`, `status`, and `message` fields. Current state: all 10 constraints pass.

**Step 3: Verify JSON parsing locally**

```bash
jq '.results | length' conformance.json
jq '[.results[] | select(.status == "Pass")] | length' conformance.json
```

Expected: Total count matches number of constraints (10+). Pass count matches total (all passing currently).

**Step 4: Clean up temporary file**

```bash
rm conformance.json
```

**Step 5: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add conformance gate with step summary"
```

---

### Task 6: Final Verification and Documentation

**Files:**
- Modify: `docs/plan/PROGRESS.md`

**Step 1: Review the complete CI workflow**

Read `.github/workflows/ci.yml` end-to-end and verify:
- Trigger: push to main + PRs to main
- Steps in order: checkout, toolchain, cache, fmt, clippy, test, audit install, audit, wasm-pack install, wasm build, node setup, npm ci, npm test, conformance check, conformance summary
- `continue-on-error: true` only on conformance check step
- `if: always()` on conformance summary step

**Step 2: Run full local verification**

Run all CI commands in sequence to confirm the complete pipeline works:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo audit
wasm-pack build crates/wasm --target web
cd web && npm ci && npm test && cd ..
cargo run --bin svt -- import design/architecture.yaml
cargo run --bin svt -- analyze .
cargo run --bin svt -- check --analysis
```

Expected: All commands pass.

**Step 3: Update PROGRESS.md**

Update the milestone table to mark M9 as complete. Update the test count, the "Current state" line, and move M9 out of "Suggested Next Milestones" into "Completed Milestones".

Add to the completed milestones table:

```
| **9** | CI Pipeline | 2026-02-19 | 282 | GitHub Actions CI, Rust fmt/clippy/test/audit, WASM build, web tests, conformance gate with step summary |
```

Update "Current state" to reflect CI is operational.

Remove M9 from "Suggested Next Milestones" section (keep M10 — Plugin Foundations).

Add to plan documents table:

```
| `2026-02-19-milestone-9-implementation.md` | M9 implementation plan (COMPLETE) |
```

**Step 4: Commit**

```bash
git add docs/plan/PROGRESS.md
git commit -m "docs: mark milestone 9 as complete with CI pipeline"
```

---

## Complete File: `.github/workflows/ci.yml`

For reference, the final assembled workflow:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    name: Build & Test
    runs-on: ubuntu-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
          targets: wasm32-unknown-unknown

      - name: Cargo cache
        uses: Swatinem/rust-cache@v2

      - name: Format check
        run: cargo fmt --check

      - name: Clippy
        run: cargo clippy -- -D warnings

      - name: Test
        run: cargo test

      - name: Install cargo-audit
        run: cargo install cargo-audit --locked

      - name: Audit
        run: cargo audit

      - name: Install wasm-pack
        run: cargo install wasm-pack --locked

      - name: WASM build
        run: wasm-pack build crates/wasm --target web

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm
          cache-dependency-path: web/package-lock.json

      - name: Web install
        run: npm ci
        working-directory: web

      - name: Web test
        run: npm test
        working-directory: web

      - name: Conformance check
        id: conformance
        continue-on-error: true
        run: |
          cargo run --bin svt -- import design/architecture.yaml
          cargo run --bin svt -- analyze .
          cargo run --bin svt -- check --analysis --format json > conformance.json

      - name: Conformance summary
        if: always() && steps.conformance.outcome != 'skipped'
        run: |
          if [ -f conformance.json ]; then
            TOTAL=$(jq '.results | length' conformance.json)
            PASS=$(jq '[.results[] | select(.status == "Pass")] | length' conformance.json)
            FAIL=$(jq '[.results[] | select(.status == "Fail")] | length' conformance.json)
            WARN=$(jq '[.results[] | select(.status == "Warn")] | length' conformance.json)
            NOT_EVAL=$(jq '[.results[] | select(.status == "NotEvaluable")] | length' conformance.json)

            echo "## Conformance Report" >> "$GITHUB_STEP_SUMMARY"
            echo "" >> "$GITHUB_STEP_SUMMARY"
            echo "| Status | Count |" >> "$GITHUB_STEP_SUMMARY"
            echo "|--------|-------|" >> "$GITHUB_STEP_SUMMARY"
            echo "| Pass | $PASS |" >> "$GITHUB_STEP_SUMMARY"
            echo "| Fail | $FAIL |" >> "$GITHUB_STEP_SUMMARY"
            echo "| Warn | $WARN |" >> "$GITHUB_STEP_SUMMARY"
            echo "| Not Evaluable | $NOT_EVAL |" >> "$GITHUB_STEP_SUMMARY"
            echo "| **Total** | **$TOTAL** |" >> "$GITHUB_STEP_SUMMARY"

            if [ "$FAIL" -gt 0 ]; then
              echo "" >> "$GITHUB_STEP_SUMMARY"
              echo "### Failures" >> "$GITHUB_STEP_SUMMARY"
              echo "" >> "$GITHUB_STEP_SUMMARY"
              jq -r '.results[] | select(.status == "Fail") | "- **\(.constraint)**: \(.message)"' conformance.json >> "$GITHUB_STEP_SUMMARY"
            fi
          else
            echo "## Conformance Report" >> "$GITHUB_STEP_SUMMARY"
            echo "" >> "$GITHUB_STEP_SUMMARY"
            echo "Conformance check did not produce output." >> "$GITHUB_STEP_SUMMARY"
          fi
```

## Complete File: `.cargo/audit.toml`

```toml
[advisories]
ignore = [
    "RUSTSEC-2025-0056",  # adler: unmaintained, used by cozo transitive dep
    "RUSTSEC-2025-0141",  # bincode: unmaintained, used by cozo transitive dep
    "RUSTSEC-2025-0057",  # fxhash: unmaintained, used by cozo transitive dep
]
```
