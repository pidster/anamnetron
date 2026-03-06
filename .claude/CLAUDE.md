# Anamnetron

A tool for designing, documenting, and validating software architecture. Rust backend with web GUI frontend.

## Principles

See @PRINCIPLES.md for the full set of development principles governing this project.

## Project Structure

Rust workspace (planned):

```
crates/
  core/        — data model, validation, conformance (compiles to WASM)
  analyzer/    — tree-sitter analysis, discovery mode
  cli/         — CLI entry point, export (Mermaid/SVG/PNG)
  server/      — API service (Axum)

web/           — frontend (TypeScript + WASM core)
```

## Documentation

```
docs/
  design/      — design documents (data model, interchange format, etc.)
  adr/         — architecture decision records
  plan/        — implementation plans
design/        — the project's own prescriptive architecture model (dog-food)
```

Key documents:
- @TECH_STACK.md — technology choices and rationale
- @docs/design/DATA_MODEL.md — graph schema, GraphStore trait, constraints
- @design/architecture.yaml — this project's own design model

## Three Modes

1. **Design mode** (prescriptive) — define intended architecture, boundaries, allowed dependencies
2. **Discovery mode** (descriptive) — static analysis of real code, deriving the actual architecture
3. **Conformance mode** (comparative) — overlay design onto discovery, detect violations and drift

## Build Commands

**Always use `./scripts/build.sh` to build the application.** Do not run `cargo build`, `wasm-pack build`, or `npm run build` individually — the build script handles the correct dependency order (WASM → Web → Rust).

```bash
./scripts/build.sh           # Full build (WASM → Web → Rust)
./scripts/build.sh --release # Release profile
./scripts/build.sh wasm      # Only WASM (crates/wasm → crates/wasm/pkg/)
./scripts/build.sh web       # Only web (web/ → web/dist/, assumes WASM pkg exists)
./scripts/build.sh rust      # Only Rust workspace
```

Other project commands (these are NOT part of the build):

```bash
cargo test               # Run all tests
cargo clippy             # Lint
cargo fmt --check        # Format check
cargo audit              # Dependency audit
cd web && npm test        # Run web tests
cargo llvm-cov           # Code coverage (text summary)
cargo llvm-cov --html    # Code coverage (HTML report in target/llvm-cov/html/)
```

## Coding Standards

- Rust 2021 edition
- `clippy` and `rustfmt` enforced (pre-commit hooks)
- Public APIs require documentation (`#[warn(missing_docs)]`)
- Property-based tests for graph operations (proptest)
- **>95% region coverage is mandatory** for new code (`cargo llvm-cov`)

## Conventions

- Prefer returning `Result` over panicking
- Use `thiserror` for library error types, `anyhow` for application error types
- Minimize dependencies — each dependency must be justified
- No `unsafe` without documented justification and review

## Design-First Workflow

**Implementation must follow validated design.** Before writing code for new features or significant changes:

1. Verify design against PRINCIPLES.md and architecture rules
2. For new APIs, traits, or data model changes — produce a design artifact (ADR, design doc, or inline comment)
3. Use `/arch-review` to validate architectural alignment before implementation
4. Use `/review` after implementation to verify quality
5. Use `/quality-check` for comprehensive quality gates

## Orchestration Model

**The main agent is an orchestrator, not an implementer.** All substantive work — code changes, test writing, reviews, architecture analysis — MUST be delegated to named agent teammates. The main agent's role is:

1. **Understand the request** — Clarify requirements with the user
2. **Plan the work** — Break tasks into discrete, assignable units
3. **Create a team** — Use TeamCreate and spawn the appropriate agents
4. **Assign tasks** — Use TaskCreate and TaskUpdate to assign work to teammates
5. **Coordinate** — Monitor progress, unblock teammates, relay information
6. **Report** — Summarize results to the user

### Delegation Rules

- **ALL code edits** go through the **implementer** agent (spawned with `mode: "plan"`)
- **Architecture reviews** go through the **architect** agent
- **Code reviews** go through the **reviewer** agent
- **Test writing** goes through the **test-writer** agent
- For tasks requiring both implementation and testing, spawn both implementer and test-writer as teammates
- Even single-task requests should be delegated — create a team with at least one teammate

### Exception: Trivial Edits

The main agent may make trivial edits (typos, single-line fixes) directly ONLY when the user explicitly instructs it or agrees to a specific request. This is the exception, not the norm.

### Team Lifecycle

1. Create a team for each user request or logical unit of work
2. Spawn teammates with appropriate agent types and `mode` settings
3. Create tasks and assign them to teammates
4. Monitor completion via task list and teammate messages
5. Shut down teammates and delete the team when work is complete
6. Report the outcome to the user

## Custom Agents

- **implementer** — Primary coding agent; writes, edits, and refactors code (runs in plan mode)
- **architect** — Architecture review against principles and design constraints
- **reviewer** — Code review for correctness, quality, and standards compliance
- **test-writer** — Comprehensive test authoring (unit, integration, property-based, web)

## Rules

All rules in `.claude/rules/` are active:
- `architecture.md` — Dependency flow, graph store primacy, WASM compatibility
- `rust.md` — Rust code style, error handling, zero-copy patterns
- `testing.md` — Test coverage, naming, property-based testing
- `web-frontend.md` — Svelte/TypeScript, Cytoscape, WASM integration
- `security.md` — Input validation, path safety, no network by default
- `code-review.md` — Review checklist (correctness, architecture, quality, performance)
- `design-first.md` — Design validation before implementation

## Quality Gates

Prompt-based hooks enforce quality at every edit:
- **Pre-edit**: Validates architecture compliance, dependency flow, WASM compatibility, design coverage
- **Pre-write**: Confirms new files are necessary and architecturally sound
- **Post-edit**: Checks for missing tests, unwrap/expect usage, doc comments, security concerns
- **Post-write**: Auto-formats Rust files with rustfmt

## Roadmap Priority (Post-M29)

Next milestones in priority order (see `docs/plan/PROGRESS.md` for full details):

1. **M30: Java Analyzer** — New language with tree-sitter-java, full structural extraction, call graph, Maven/Gradle discovery
