Perform an architecture review of the current changes or a specified area of the codebase.

## Steps

1. Identify the scope — if reviewing changes, run `git diff`; if reviewing a module, read its source
2. Read PRINCIPLES.md and TECH_STACK.md for alignment context
3. Read the relevant design docs in docs/design/ and docs/adr/
4. Check the prescriptive design model in design/architecture.yaml
5. Verify:
   - Dependency flow: cli/server → analyzer → core (never reverse)
   - WASM compatibility: core/ has no platform-specific dependencies
   - Graph store primacy: graph store is the canonical data representation
   - Three-mode model: changes align with design/discovery/conformance
   - Plugin boundaries: plugin API surface is stable and narrow
6. Check if the change requires an ADR or design doc update

## Output

Structured review:
- **Principle alignment** — Pass/Concern for each affected principle
- **Dependency flow** — Any violations
- **WASM impact** — Whether core WASM compatibility is affected
- **Design doc status** — Whether docs need creating or updating
- **Dog-food impact** — Whether design/architecture.yaml needs updating
- **Recommendations** — Specific suggestions
