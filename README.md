# Software Visualizer Tool (SVT)

A tool for designing, documenting, and validating software architecture. Define your intended architecture, analyze your actual codebase, and detect drift between the two.

SVT operates in three modes:

- **Design** -- define your intended architecture as a YAML model: components, boundaries, allowed dependencies, and constraints.
- **Discovery** -- analyze real source code with tree-sitter to derive the actual architecture automatically.
- **Conformance** -- compare design against discovery to detect violations, missing implementations, and undocumented components.

## Quick Start

### Install

Build from source (requires [Rust](https://rustup.rs/)):

```bash
cargo build --release
```

The binary is at `target/release/svt`.

### Analyze a project

Point SVT at any supported project to discover its architecture:

```bash
svt analyze /path/to/project
```

This scans the source tree using tree-sitter, extracts structure (crates, modules, functions, classes), and stores the result as an analysis snapshot in `.svt/store`.

### Define a design model

Create a `design/architecture.yaml` describing your intended architecture:

```yaml
format: svt/v1
kind: design
version: 1

nodes:
  - canonical_path: /my-system
    kind: system
    sub_kind: workspace
    name: my-system
    children:
      - canonical_path: /my-system/api
        kind: service
        sub_kind: crate
        name: api
      - canonical_path: /my-system/core
        kind: service
        sub_kind: crate
        name: core

edges:
  - source: /my-system/api
    target: /my-system/core
    kind: depends

constraints:
  - name: core-no-api-deps
    kind: must_not_depend
    scope: /my-system/core/**
    target: /my-system/api/**
    message: "Core must not depend on API"
    severity: error
```

Import it into the store:

```bash
svt import design/architecture.yaml
```

### Check conformance

Compare your design against the analyzed codebase:

```bash
svt check --analysis
```

SVT evaluates every constraint and reports passes, failures, unimplemented design nodes, and undocumented analysis nodes.

### Export a diagram

Generate a visual representation of your architecture:

```bash
svt export --format mermaid
svt export --format dot -o architecture.dot
svt export --format svg -o architecture.svg
svt export --format png -o architecture.png
```

## Usage Guide

### The Design Model

Design models are YAML or JSON files that describe your intended architecture. The format uses four concepts:

**Nodes** represent software elements at varying levels of abstraction:

| Kind | Description | Examples |
|------|-------------|----------|
| `system` | Top-level workspace or repository | Monorepo, workspace |
| `service` | Deployable unit or package | Crate, npm package, Go module |
| `component` | Module or namespace within a service | Rust module, TypeScript directory |
| `unit` | Leaf element | Function, struct, class, trait |

Each node has a **canonical path** -- a language-neutral identifier in kebab-case (e.g., `/my-system/api/handlers/create-order`). Nodes can be nested via `children` to form a hierarchy.

**Edges** describe relationships between nodes:

| Kind | Meaning |
|------|---------|
| `depends` | Compile-time or runtime dependency |
| `calls` | Direct invocation |
| `contains` | Hierarchical parent-child (created automatically from `children`) |
| `implements` | Interface implementation |
| `extends` | Inheritance |
| `exports` | Public API surface |
| `data_flow` | Data movement between components |

**Constraints** are assertions about architectural properties:

| Kind | What it checks |
|------|----------------|
| `must_not_depend` | No dependency edges from `scope` to `target` |
| `must_only_depend` | Dependencies from `scope` only go to `target` |
| `boundary` | Encapsulation -- internal details don't leak |
| `layer_order` | Dependency direction follows a layer ordering |
| `must_contain` | A node must have specific children |
| `max_fan_out` | Maximum outgoing dependencies |
| `max_fan_in` | Maximum incoming dependencies |

Each constraint has a severity (`error`, `warning`, or `info`) that determines whether it causes a non-zero exit code.

**Metadata** on nodes and edges supports arbitrary key-value pairs for documentation.

### Discovery Mode

SVT uses tree-sitter to analyze source code in multiple languages:

| Language | Detected from | Extracted structure |
|----------|---------------|---------------------|
| Rust | `Cargo.toml` | Crates, modules, structs, enums, traits, functions, impls |
| TypeScript | `package.json` | Packages, modules, classes, interfaces, functions |
| Go | `go.mod` | Modules, packages, structs, interfaces, functions |
| Python | `pyproject.toml` | Packages, modules, classes, functions |

Run discovery with:

```bash
svt analyze .                          # Current directory
svt analyze /path/to/project           # Specific path
svt analyze . --commit-ref abc123      # Tag snapshot with a git commit
```

The analyzer automatically detects the current git HEAD if `--commit-ref` is not provided.

### Conformance Checking

Conformance compares a design model against an analysis snapshot:

```bash
# Check the design model on its own (validates constraints against design nodes)
svt check

# Compare design against the latest analysis
svt check --analysis

# Compare specific versions
svt check --design 1 --analysis 2

# Machine-readable output for CI
svt check --analysis --format json

# Fail on warnings (default: only errors cause non-zero exit)
svt check --analysis --fail-on warning
```

The conformance report includes:
- **Constraint results**: pass/fail for each constraint, with violation details
- **Unimplemented nodes**: design nodes with no matching analysis node
- **Undocumented nodes**: analysis nodes with no matching design node
- **Summary**: total counts of passed, failed, warned, and not-evaluable constraints

### Snapshot Diffing

Compare two snapshots to see what changed:

```bash
svt diff --from 1 --to 2              # Human-readable diff
svt diff --from 1 --to 2 --format json # Machine-readable diff
```

Shows added, removed, and changed nodes and edges between versions.

### Export Formats

```bash
svt export --format mermaid            # Mermaid flowchart (stdout)
svt export --format json               # Full JSON interchange format
svt export --format dot                # Graphviz DOT
svt export --format svg -o out.svg     # SVG (requires Graphviz `dot` on PATH)
svt export --format png -o out.png     # PNG (requires Graphviz `dot` on PATH)
svt export --format json --version 2   # Export a specific snapshot version
```

Export formats are pluggable -- plugins can register additional formats.

### Plugins

SVT supports dynamic plugins that add custom constraint evaluators and export formats.

Plugins are loaded from three locations (in order):
1. Explicit paths via `--plugin <path>`
2. Project-local directory: `.svt/plugins/`
3. User-global directory: `~/.svt/plugins/`

List loaded plugins:

```bash
svt plugin list
```

### Web GUI

SVT includes a web-based frontend built with Svelte and Cytoscape.js for interactive graph visualization.

Start the server:

```bash
svt-server
```

The server provides a REST API and serves the web UI. Key API endpoints:

| Endpoint | Description |
|----------|-------------|
| `GET /api/health` | Health check |
| `GET /api/snapshots` | List all snapshots |
| `GET /api/snapshots/{version}/nodes` | List nodes in a snapshot |
| `GET /api/snapshots/{version}/edges` | List edges (optional `?kind=` filter) |
| `GET /api/snapshots/{version}/graph` | Full graph in Cytoscape.js format |
| `GET /api/search?path=GLOB&version=V` | Search nodes by canonical path |
| `GET /api/conformance/design/{version}` | Evaluate design constraints |
| `GET /api/conformance?design=V&analysis=V` | Design-vs-analysis conformance |
| `GET /api/diff?from=V1&to=V2` | Snapshot diff |

### CI Integration

SVT is designed for CI pipelines. A typical workflow:

```bash
# Import the design model
svt import design/architecture.yaml

# Analyze the codebase
svt analyze .

# Run conformance -- non-zero exit on failure
svt check --analysis --format json --fail-on error
```

Exit codes: `0` = all constraints at or above the threshold pass, `1` = at least one failure.

The `--format json` output is suitable for parsing in CI scripts or generating step summaries.

### Store

SVT stores all data in `.svt/store` (configurable via `--store`). The store is a local CozoDB database containing nodes, edges, constraints, and snapshot metadata. It can be safely deleted and recreated from design files and fresh analysis runs.

## Developer Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (2021 edition, stable)
- [Node.js](https://nodejs.org/) 22+ (for the web frontend)
- [Graphviz](https://graphviz.org/) (optional, for SVG/PNG export)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) (optional, for WASM builds)

### Build

```bash
cargo build                            # Build all crates
```

### Test

```bash
cargo test                             # Run all tests
cargo test -p svt-core                 # Test a specific crate
```

Tests include property-based testing (proptest) for graph operations and serialization round-trips.

### Lint and Format

```bash
cargo clippy                           # Lint
cargo fmt --check                      # Check formatting
cargo audit                            # Dependency audit
```

### Project Structure

```
crates/
  core/        Data model, graph store, validation, conformance (WASM-compatible)
  analyzer/    Tree-sitter analysis, discovery mode
  cli/         CLI entry point (svt binary)
  server/      Axum REST API, serves the web UI
  wasm/        WASM bridge to core for browser-side queries

web/           Svelte + Cytoscape.js frontend
design/        This project's own design model (dog-food)
docs/
  design/      Design documents (data model, interchange format)
  adr/         Architecture decision records
  plan/        Implementation plans
```

Crate dependencies flow inward: `cli`/`server` -> `analyzer` -> `core`. Core has no outward dependencies and compiles to WASM.

### Web Frontend

```bash
cd web
npm install
npm run build                          # Production build -> web/dist/
npm run dev                            # Development server
npm test                               # Run frontend tests
```

### WASM Build

```bash
wasm-pack build crates/wasm --target web
```

### Dog-fooding

SVT validates its own architecture in CI. The project's design model lives at `design/architecture.yaml` and conformance is checked on every push:

```bash
svt import design/architecture.yaml
svt analyze .
svt check --analysis
```

### Writing Plugins

Implement the `SvtPlugin` trait from `svt-core`:

```rust
use svt_core::plugin::{SvtPlugin, declare_plugin};

struct MyPlugin;

impl SvtPlugin for MyPlugin {
    fn name(&self) -> &str { "my-plugin" }
    fn version(&self) -> &str { "0.1.0" }
    fn api_version(&self) -> u32 { 1 }

    fn constraint_evaluators(&self) -> Vec<Box<dyn ConstraintEvaluator>> {
        // Return custom constraint evaluators
        vec![]
    }

    fn export_formats(&self) -> Vec<Box<dyn ExportFormat>> {
        // Return custom export formats
        vec![]
    }
}

declare_plugin!(MyPlugin);
```

Build as a shared library (`cdylib`) and place the `.dylib`/`.so`/`.dll` in `.svt/plugins/` or `~/.svt/plugins/`.
