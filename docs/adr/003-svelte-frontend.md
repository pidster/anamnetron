# ADR-003: Svelte as Frontend Framework

## Status

Accepted

## Context

The frontend is primarily a rendering layer over a graph visualisation library (Cytoscape.js) and WASM core logic. Heavy computation happens in WASM, not in the UI framework. The framework choice should minimise overhead and not constrain the visualisation layer.

## Decision

Use Svelte as the frontend framework.

## Alternatives Considered

- **React** — largest ecosystem, most graph visualisation library integrations. Heavier bundle, more boilerplate. Ecosystem advantage is less important when the heavy logic lives in WASM.
- **Vue** — good developer experience, middle ground in bundle size and ecosystem. Less momentum in the visualisation space.

## Consequences

- Minimal framework overhead — Svelte compiles to vanilla JS with no virtual DOM.
- Cytoscape.js is framework-agnostic and works cleanly with Svelte.
- Smaller community than React, but sufficient for our needs since the framework is a thin rendering layer.
- WASM integration via wasm-bindgen works equally well regardless of framework choice.
