# Design-First Development

Implementation must follow a validated design. This rule governs how new features and significant changes are approached.

## When Design Is Required

A design step is required before implementation when ANY of the following apply:

- Adding a new public API, trait, or data model type
- Introducing a new crate, module, or architectural component
- Changing the graph schema (node kinds, edge kinds, properties)
- Adding a new language analyzer or plugin type
- Modifying the dependency flow between crates
- Adding a new export format or visualization type
- Any change that touches more than 3 files in core/

## Design Validation Checklist

Before implementing, verify the design against:

1. **PRINCIPLES.md** — Does it align with usability, portability, extensibility, data sovereignty?
2. **Architecture rules** — Does it respect the dependency flow and WASM compatibility of core?
3. **Data model** — Is the graph store used as the primary representation? Are new node/edge kinds justified?
4. **Existing patterns** — Does it follow established conventions or does it introduce a new pattern? If new, is it justified?
5. **Testing strategy** — How will this be tested? Are property-based tests applicable?
6. **Backward compatibility** — Does it break existing APIs, file formats, or plugin contracts?

## Design Artifacts

For significant changes, produce one of:

- **ADR** (Architecture Decision Record) in `docs/adr/` — for architectural choices with trade-offs
- **Design doc** in `docs/design/` — for data model changes, protocol designs, or complex features
- **Inline design comment** — for smaller decisions that don't warrant a separate document

## Dog-Fooding

The project's own architecture model lives in `design/`. When adding architectural components, update the design model and verify conformance. The tool validates its own architecture — changes that violate the design model must either fix the code or update the model with justification.
