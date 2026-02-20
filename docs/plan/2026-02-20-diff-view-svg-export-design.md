# Web UI Diff View + SVG/PNG Export — Design

## Feature 1: Web UI Diff View

**Goal:** Visualize differences between two analysis snapshots as a color-coded overlay on the existing graph.

**UX:** A "Compare to..." dropdown appears next to the existing snapshot selector. When a comparison version is selected, the app calls `getDiff(current, compare)` and applies CSS classes to Cytoscape elements:

- **Added nodes** — green border + badge (`.diff-added`)
- **Removed nodes** — red border, rendered as ghost nodes (`.diff-removed`)
- **Changed nodes** — amber border + badge (`.diff-changed`)
- **Added/removed edges** — colored lines matching the same scheme

A summary banner shows counts ("3 added, 1 removed, 2 changed"). Clearing the dropdown returns to normal view. Diff state is encoded in the URL hash (`#v=2&diff=1`) for permalinks.

**Architecture:** Follows the existing conformance overlay pattern — CSS classes on Cytoscape elements. The diff store manages diff state alongside the existing graph store. Removed nodes need to be fetched from the comparison snapshot and merged into the current graph as ghost elements.

**Components affected:**
- `App.svelte` — add comparison dropdown, diff summary banner
- `GraphView.svelte` — add diff CSS classes to stylesheet, merge ghost nodes
- `stores/graph.ts` — add diff state management
- `lib/router.ts` — add `diff` parameter to URL hash parsing

## Feature 2: SVG/PNG Export via Graphviz

**Goal:** Add `svt export --format svg` and `svt export --format png` using Graphviz CLI piping.

**Architecture:** Two new `ExportFormat` implementations that reuse the existing `to_dot()` function and pipe through the system `dot` command.

- `SvgExporter` — `to_dot()` → `dot -Tsvg` → SVG string
- `PngExporter` — `to_dot()` → `dot -Tpng` → binary output

Both registered in `ExportRegistry::with_defaults()`. Graceful error if Graphviz is not installed.

**Data flow:** `GraphStore → to_dot() → Command::new("dot") → SVG/PNG`

**CLI:** `svt export --format svg -o arch.svg` / `svt export --format png -o arch.png`

**Error handling:** Check for `dot` command existence, return clear error message with install URL if missing.
