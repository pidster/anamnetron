# Web Frontend Conventions

## Technology

- Svelte 4 with TypeScript (strict mode)
- Cytoscape.js for interactive graph visualization
- Mermaid for diagram generation
- WASM core (svt-core compiled via wasm-bindgen) for shared validation/conformance logic
- Vite for build tooling, Vitest for testing

## Code Style

- All component props must be typed — no `any` or untyped props
- Use Svelte stores for shared state; avoid prop drilling beyond two levels
- Component files follow `PascalCase.svelte` naming
- TypeScript files follow `camelCase.ts` naming
- Keep components focused — split when a component exceeds ~200 lines or handles multiple concerns
- Use `$:` reactive declarations sparingly; prefer stores for complex derived state
- CSS is scoped per component — avoid `:global` unless styling third-party library elements

## Graph Visualization

- Cytoscape graph interactions (tap, hover, select) must have keyboard-accessible equivalents
- Graph layouts are deterministic given the same input — use seeded layouts where randomness is involved
- Large graphs (>500 nodes) must use virtualization or progressive rendering
- Conformance overlays (pass/fail/warning) use consistent, accessible colour coding with shape/icon fallbacks

## WASM Integration

- WASM module is loaded asynchronously — components must handle the loading state
- All WASM calls go through a typed wrapper in `web/src/wasm/` — never call `wasm_bindgen` exports directly from components
- WASM errors are caught and surfaced to the user, never silently swallowed

## Testing

- Component tests use Vitest with `@testing-library/svelte`
- Test user-visible behaviour, not implementation details
- Graph visualization tests use snapshot comparisons of Cytoscape JSON state
- Accessibility: test keyboard navigation and screen reader compatibility for key workflows
