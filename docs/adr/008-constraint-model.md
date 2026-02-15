# ADR-008: Declarative Constraint Model

## Status

Accepted

## Context

The tool's conformance mode needs to evaluate architectural rules against the analysis graph. Rules must be: human-authorable alongside design models, evaluatable by the graph store, and reportable with evidence (violations with source references). The constraint system should be extensible without requiring a separate rules engine.

## Decision

Constraints are first-class entities in the graph, versioned alongside design nodes. Six constraint kinds are defined initially: `must_not_depend`, `must_only_depend`, `boundary`, `layer_order`, `must_contain`, and `max_fan_out`/`max_fan_in`. Each kind maps to a Datalog query pattern for evaluation. Constraints carry severity levels (`error`, `warning`, `info`) that map to CLI exit codes for CI integration.

## Alternatives Considered

- **External rules engine** — a separate DSL or rules framework (e.g., OPA/Rego). Adds a dependency and a second query language. More powerful, but more complex.
- **Hardcoded rules** — rules baked into Rust code. Not user-configurable. Violates the extensibility principle.
- **Graph-embedded policies (edges with "deny" kind)** — simpler to model but less expressive. Can't represent layer ordering or structural requirements.

## Consequences

- Constraints are stored, versioned, and exported in the interchange format — they travel with the design model.
- CozoDB's Datalog makes constraint evaluation a natural query, not a separate system.
- The six initial constraint kinds cover common architectural rules. More kinds can be added without schema changes.
- User-defined rules (future goal) could be expressed as custom Datalog queries — the architecture doesn't preclude this.
- Severity levels enable CI integration: `svt check --fail-on=error`.

See [docs/design/DATA_MODEL.md](../design/DATA_MODEL.md) for the constraint schema and evaluation details.
