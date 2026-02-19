# Milestone 9: CI Pipeline — Design

## Goal

Set up a GitHub Actions CI workflow that builds, tests, lints, and audits the full stack (Rust + WASM + web frontend) on every push and PR, with a conformance dog-food check as a visible but non-blocking gate.

## Scope

- Single GitHub Actions workflow (`.github/workflows/ci.yml`)
- Linux-only (`ubuntu-latest`) — add more platforms later if needed
- Full Rust checks: fmt, clippy, test, audit
- WASM compilation via wasm-pack
- Web frontend: npm ci, vitest
- Conformance check: import design, analyze project, compare — warn but don't block
- Audit ignore config for known CozoDB transitive dependency warnings

## Workflow Architecture

**Trigger:** Push to `main`, pull requests to `main`

**Single job with sequential steps** (shares build cache):

| Step | Command | Failure behavior |
|------|---------|-----------------|
| Checkout | `actions/checkout@v4` | Fail |
| Rust toolchain | `dtolnay/rust-toolchain@stable` + clippy, rustfmt, wasm32-unknown-unknown | Fail |
| Cargo cache | `Swatinem/rust-cache@v2` | Soft (no cache = cold build) |
| Format check | `cargo fmt --check` | Fail |
| Clippy | `cargo clippy -- -D warnings` | Fail |
| Test | `cargo test` | Fail |
| Audit | `cargo audit` | Fail |
| WASM build | `cargo install wasm-pack && wasm-pack build crates/wasm --target web` | Fail |
| Node.js setup | `actions/setup-node@v4` with node 22 | Fail |
| Web install | `npm ci` (in `web/`) | Fail |
| Web test | `npm test` (in `web/`) | Fail |
| Conformance | `svt import` + `svt analyze` + `svt check --analysis` | **continue-on-error** |
| Conformance summary | Parse JSON output into `$GITHUB_STEP_SUMMARY` | Always run |

## Conformance Gate

The conformance step runs as a visibility tool, not a hard gate:

1. Build `svt` CLI (cached from test step)
2. `cargo run --bin svt -- import design/architecture.yaml`
3. `cargo run --bin svt -- analyze .`
4. `cargo run --bin svt -- check --analysis --format json`
5. Parse results into GitHub step summary (pass/fail/warn counts, violations)

Set `continue-on-error: true` so CI stays green even when architecture drifts intentionally during development. Can be promoted to a hard gate later by removing `continue-on-error`.

## Audit Configuration

CozoDB 0.7 has 3 unmaintained (but not vulnerable) transitive dependencies:
- `adler` (RUSTSEC-2025-0056) — replaced by `adler2`
- `bincode` (RUSTSEC-2025-0141) — v1 unmaintained
- `fxhash` (RUSTSEC-2025-0057) — no longer maintained

Create `.cargo/audit.toml` to ignore these known warnings so CI stays green:

```toml
[advisories]
ignore = [
    "RUSTSEC-2025-0056",  # adler: unmaintained, used by cozo transitive dep
    "RUSTSEC-2025-0141",  # bincode: unmaintained, used by cozo transitive dep
    "RUSTSEC-2025-0057",  # fxhash: unmaintained, used by cozo transitive dep
]
```

## Caching Strategy

- **Rust:** `Swatinem/rust-cache@v2` caches `~/.cargo` and `target/`. All Rust steps benefit from shared compilation.
- **wasm-pack:** Installed via `cargo install wasm-pack` — cached by rust-cache.
- **Node.js:** `actions/setup-node@v4` with built-in npm cache.

## Expected CI Time

~5-7 minutes: Rust build ~3min, tests ~2min, WASM build ~1min, web tests ~30s.

## New Files

```
.github/workflows/ci.yml    — CI workflow
.cargo/audit.toml            — Audit ignore list for known warnings
```

## Out of Scope

- Cross-platform CI (macOS, Windows) — add later if needed
- Artifact uploads (binaries, WASM package)
- Release automation
- Plugin foundations — deferred to milestone 10
