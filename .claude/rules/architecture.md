# Architecture Guidelines

- The graph store is the primary data representation — file formats (YAML, JSON) are for interchange
- Design models (prescriptive) and analysis results (descriptive) share the same data model with provenance metadata
- Core logic (data model, validation, conformance) must compile to WASM — no platform-specific dependencies in core
- Plugin API boundaries are the hard boundary between open-source and core — keep them clean and stable
- Crate dependencies flow inward: cli/server -> analyzer -> core. Never the reverse.
- The web GUI is the primary user interface — CLI is additive
- No network calls during normal operation — external access is strictly opt-in
- All on-disk formats must be human-readable and git-friendly for interchange purposes
