# Anamnetron — Development Principles

## 1. Usability

The tool should be immediately useful with zero configuration. A developer should be able to install a single binary, point it at a project, and see a meaningful visualization within minutes. Advanced features (design models, conformance rules, custom views) are progressively discoverable, not prerequisites.

## 2. Portability

The tool runs locally on macOS, Linux, and Windows with no network dependency. The data model is defined by its schema, not by a file format — the graph store is the primary representation. Import/export supports open, human-readable, git-friendly formats (YAML, JSON, etc.) for interchange and version control. The tool never requires source code to leave the user's machine. The web GUI is the primary user interface. CLI and container deployment are additive modes.

## 3. Extensibility

The tool is extensible through a plugin model with a small, stable API surface. Language analyzers, node/edge types, and import/export formats are all pluggable. The tool ships with a useful set of built-in capabilities but does not assume it can cover every language or domain. The built-in UI provides well-designed navigation, filtering, and selection across core views (architecture, code structure, data flow). View configuration is serializable and shareable. Custom view types and user-defined conformance rules are future goals — the architecture should not preclude them, but they are not v1 deliverables. Plugin contracts are narrow, versioned, and documented.

## 4. Data Sovereignty

All data is self-hostable by default. The tool has no cloud dependency, collects no telemetry by default, and makes no external network calls during normal operation. Users own all artifacts the tool produces. Teams share data through their existing version control workflows. The architecture should support a hosted/SaaS deployment model, but self-hosted and local-first operation is the primary mode. Any optional network features (plugin registry, update checks, SaaS offering) are strictly opt-in.

## 5. Developer Experience

The tool is used to design and validate its own architecture — the project's design model lives in the repo and conformance is checked in CI. The plugin API and extension interfaces are open source to enable community contribution. Licensing for the core tool is deferred — the architecture should support both open source and source-available/commercial models. The development workflow requires only standard Rust tooling — `cargo build` produces a working tool with no external dependencies. Builds are reproducible. The codebase is structured so that contributors can add language analyzers, exporters, and plugins without deep knowledge of core internals.

## 6. Quality / Engineering Standards

Core logic (data model, graph operations, conformance) has thorough test coverage including property-based tests. All layers target high test coverage — thoroughness is preferred over speed of delivery. Pre-commit hooks enforce lint, format, and audit checks locally before code reaches CI. CI runs on every PR: build, test, lint, format, dependency audit, and cross-platform verification (macOS, Linux, Windows). The project's own design model is validated in CI as a dog-food check. Dependencies are minimal, audited, and justified. Code follows standard Rust conventions enforced by clippy and rustfmt. Public APIs are documented.

## 7. Interoperability

The tool complements existing developer workflows rather than replacing them. CI integration is first-class — conformance results are available as exit codes and machine-readable output. External knowledge sources (git repos, wikis, API specs, architecture DSLs) are treated as live inputs, not one-shot imports. Sources are monitored for changes and the user is notified when upstream knowledge changes. Export formats are pluggable — the core ships with Mermaid, SVG/PNG, and JSON. The data model draws on established architectural description practices (particularly C4's levels of abstraction) without being locked to any single framework. IDE integration is not precluded but is not a v1 goal.
