# ADR-004: Cytoscape.js for Graph Visualisation

## Status

Accepted

## Context

The tool needs interactive graph visualisation with:
- Multiple layout algorithms (hierarchical, force-directed, grid)
- Drill-down navigation (expand/collapse compound nodes)
- Conditional styling (conformance colouring: green/red/grey)
- Zoom, pan, and selection
- Canvas-based rendering for performance at scale

## Decision

Use Cytoscape.js as the graph visualisation library.

## Alternatives Considered

- **D3.js** — maximum visual control, any visualisation possible. Very low-level — requires building layout, interaction, and rendering from scratch. Weeks of work to match Cytoscape's built-in capabilities.
- **React Flow** — React-native, good for flowchart-style node editors. More focused on flow diagrams than general graph exploration. React-specific.
- **ELK (Eclipse Layout Kernel)** — best-in-class hierarchical layout algorithms. Layout engine only, no rendering. Can be added as a Cytoscape layout plugin if needed.

## Consequences

- Compound nodes support drill-down navigation out of the box.
- Multiple layout algorithms available, with ELK as a plugin for hierarchical views.
- Rich styling API supports conditional conformance colouring.
- Canvas-based rendering handles medium-to-large graphs.
- Styling configuration can be verbose — mitigated by building a styling layer that maps conformance status to Cytoscape styles.
