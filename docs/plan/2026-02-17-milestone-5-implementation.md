# Milestone 5: Svelte Web Frontend Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Svelte 5 + Cytoscape.js frontend consuming the Milestone 4 REST API, providing an interactive architecture graph explorer.

**Architecture:** Single-page panel-based UI. App loads snapshots, user picks a version, fetches the Cytoscape.js-format graph from `/api/snapshots/{v}/graph`, renders it with compound node support. Clicking nodes shows detail in a side panel. Conformance overlay colours nodes by status. No WASM — all data from server API over HTTP.

**Tech Stack:** Svelte 5, TypeScript, Vite, Cytoscape.js, cytoscape-cose-bilkent, cytoscape-dagre, Vitest

**Design doc:** `docs/plan/2026-02-17-milestones-4-5-design.md`

**Dependency Graph:**
```
Task 0 (scaffold) ──→ Task 1 (types+api) ──→ Task 2 (stores)
                                                   │
                                              Task 3 (GraphView)
                                                   │
                                              Task 4 (NodeDetail)
                                                   │
                                              Task 5 (SnapshotSelector+Search)
                                                   │
                                              Task 6 (ConformanceReport)
                                                   │
                                              Task 7 (App layout + integration)
                                                   │
                                              Task 8 (server static serving)
                                                   │
                                              Task 9 (verification + dog-food)
```

---

### Task 0: Scaffold Svelte + Vite Project

**Files:**
- Modify: `web/package.json`
- Create: `web/tsconfig.json`
- Create: `web/vite.config.ts`
- Create: `web/index.html`
- Create: `web/src/main.ts`
- Create: `web/src/App.svelte`
- Create: `web/src/app.css`
- Create: `web/.gitignore`

**Step 1: Install dependencies**

Run from `web/`:
```bash
npm install svelte cytoscape cytoscape-cose-bilkent cytoscape-dagre
npm install -D typescript vite @sveltejs/vite-plugin-svelte vitest @types/cytoscape svelte-check
```

**Step 2: Create web/.gitignore**

```
node_modules/
dist/
.vite/
```

**Step 3: Update package.json scripts**

After npm install, update `web/package.json` to add scripts:

```json
{
  "name": "software-visualizer-web",
  "version": "0.1.0",
  "private": true,
  "description": "Svelte frontend for software-visualizer-tool",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "check": "svelte-check --tsconfig ./tsconfig.json",
    "test": "vitest run",
    "test:watch": "vitest"
  }
}
```

Note: `npm install` will add dependencies and devDependencies sections automatically. Just add `"type": "module"` and the scripts block.

**Step 4: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ESNext",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "verbatimModuleSyntax": true,
    "lib": ["ESNext", "DOM", "DOM.Iterable"],
    "types": ["vitest/globals"]
  },
  "include": ["src/**/*.ts", "src/**/*.svelte"]
}
```

**Step 5: Create vite.config.ts**

```typescript
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  server: {
    proxy: {
      "/api": {
        target: "http://localhost:3000",
        changeOrigin: true,
      },
    },
  },
  build: {
    outDir: "dist",
  },
  test: {
    globals: true,
    environment: "jsdom",
  },
});
```

This proxies `/api/*` requests to `svt-server` during development.

**Step 6: Create index.html**

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Software Visualizer</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

**Step 7: Create src/app.css**

```css
:root {
  --bg: #1a1a2e;
  --surface: #16213e;
  --border: #0f3460;
  --text: #e0e0e0;
  --text-muted: #8899aa;
  --accent: #53a8b6;
  --pass: #4caf50;
  --fail: #f44336;
  --warn: #ff9800;
  --muted: #607d8b;
}

* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  background: var(--bg);
  color: var(--text);
  height: 100vh;
  overflow: hidden;
}

#app {
  height: 100vh;
  display: flex;
  flex-direction: column;
}
```

**Step 8: Create src/main.ts**

```typescript
import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";

const app = mount(App, { target: document.getElementById("app")! });

export default app;
```

**Step 9: Create src/App.svelte**

```svelte
<script lang="ts">
  // Placeholder — will be replaced in Task 7
</script>

<main>
  <h1>Software Visualizer</h1>
  <p>Loading...</p>
</main>

<style>
  main {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    flex-direction: column;
    gap: 1rem;
  }
</style>
```

**Step 10: Verify build**

Run from `web/`:
```bash
npm run build
```
Expected: success, produces `web/dist/` with index.html and JS bundle.

**Step 11: Commit**

```bash
git add web/
git commit -m "feat(web): scaffold Svelte 5 + Vite project with Cytoscape.js deps"
```

---

### Task 1: TypeScript Types and API Client

**Files:**
- Create: `web/src/lib/types.ts`
- Create: `web/src/lib/api.ts`
- Create: `web/src/lib/__tests__/api.test.ts`

These types mirror the server API response shapes exactly.

**Step 1: Create types.ts**

```typescript
/** Snapshot version number. */
export type Version = number;

/** Snapshot kinds matching the server enum. */
export type SnapshotKind = "design" | "analysis" | "import";

/** Node kinds matching the server enum. */
export type NodeKind = "system" | "service" | "component" | "unit";

/** Edge kinds matching the server enum. */
export type EdgeKind =
  | "contains"
  | "depends"
  | "calls"
  | "implements"
  | "extends"
  | "data_flow"
  | "exports";

/** Provenance types. */
export type Provenance = "design" | "analysis" | "import" | "inferred";

/** Severity levels. */
export type Severity = "error" | "warning" | "info";

/** Constraint evaluation status. */
export type ConstraintStatus = "pass" | "fail" | "not_evaluable";

/** GET /api/snapshots response item. */
export interface Snapshot {
  version: Version;
  kind: SnapshotKind;
  commit_ref: string | null;
}

/** GET /api/snapshots/{v}/nodes response item (also from search). */
export interface ApiNode {
  id: string;
  canonical_path: string;
  qualified_name: string | null;
  kind: NodeKind;
  sub_kind: string;
  name: string;
  language: string | null;
  provenance: Provenance;
  source_ref: string | null;
  metadata: Record<string, unknown> | null;
}

/** GET /api/snapshots/{v}/edges response item. */
export interface ApiEdge {
  id: string;
  source: string;
  target: string;
  kind: EdgeKind;
  provenance: Provenance;
  metadata: Record<string, unknown> | null;
}

/** Cytoscape node data from /graph endpoint. */
export interface CyNodeData {
  id: string;
  label: string;
  kind: string;
  sub_kind: string;
  canonical_path: string;
  parent?: string;
  language?: string;
  source_ref?: string;
}

/** Cytoscape edge data from /graph endpoint. */
export interface CyEdgeData {
  id: string;
  source: string;
  target: string;
  kind: string;
}

/** GET /api/snapshots/{v}/graph response. */
export interface CytoscapeGraph {
  elements: {
    nodes: Array<{ data: CyNodeData }>;
    edges: Array<{ data: CyEdgeData }>;
  };
}

/** Conformance violation. */
export interface Violation {
  source_path: string;
  target_path: string | null;
  edge_id: string | null;
  edge_kind: EdgeKind | null;
  source_ref: string | null;
}

/** Constraint evaluation result. */
export interface ConstraintResult {
  constraint_name: string;
  constraint_kind: string;
  status: ConstraintStatus;
  severity: Severity;
  message: string;
  violations: Violation[];
}

/** Unmatched node in conformance report. */
export interface UnmatchedNode {
  canonical_path: string;
  kind: NodeKind;
  name: string;
}

/** Conformance summary counts. */
export interface ConformanceSummary {
  passed: number;
  failed: number;
  warned: number;
  not_evaluable: number;
  unimplemented: number;
  undocumented: number;
}

/** GET /api/conformance response. */
export interface ConformanceReport {
  design_version: Version;
  analysis_version: Version | null;
  constraint_results: ConstraintResult[];
  unimplemented: UnmatchedNode[];
  undocumented: UnmatchedNode[];
  summary: ConformanceSummary;
}

/** API error response. */
export interface ApiError {
  error: string;
}
```

**Step 2: Create api.ts**

```typescript
import type {
  Snapshot,
  ApiNode,
  ApiEdge,
  CytoscapeGraph,
  ConformanceReport,
  Version,
} from "./types";

const BASE = "";

async function fetchJson<T>(url: string): Promise<T> {
  const response = await fetch(url);
  if (!response.ok) {
    const body = await response.json().catch(() => ({ error: response.statusText }));
    throw new Error(body.error || `HTTP ${response.status}`);
  }
  return response.json();
}

/** GET /api/health */
export function getHealth(): Promise<{ status: string }> {
  return fetchJson(`${BASE}/api/health`);
}

/** GET /api/snapshots */
export function getSnapshots(): Promise<Snapshot[]> {
  return fetchJson(`${BASE}/api/snapshots`);
}

/** GET /api/snapshots/{v}/nodes */
export function getNodes(version: Version): Promise<ApiNode[]> {
  return fetchJson(`${BASE}/api/snapshots/${version}/nodes`);
}

/** GET /api/snapshots/{v}/nodes/{id} */
export function getNode(version: Version, id: string): Promise<ApiNode> {
  return fetchJson(`${BASE}/api/snapshots/${version}/nodes/${encodeURIComponent(id)}`);
}

/** GET /api/snapshots/{v}/nodes/{id}/children */
export function getChildren(version: Version, id: string): Promise<ApiNode[]> {
  return fetchJson(`${BASE}/api/snapshots/${version}/nodes/${encodeURIComponent(id)}/children`);
}

/** GET /api/snapshots/{v}/nodes/{id}/ancestors */
export function getAncestors(version: Version, id: string): Promise<ApiNode[]> {
  return fetchJson(`${BASE}/api/snapshots/${version}/nodes/${encodeURIComponent(id)}/ancestors`);
}

/** GET /api/snapshots/{v}/nodes/{id}/dependencies */
export function getDependencies(version: Version, id: string): Promise<ApiEdge[]> {
  return fetchJson(`${BASE}/api/snapshots/${version}/nodes/${encodeURIComponent(id)}/dependencies`);
}

/** GET /api/snapshots/{v}/nodes/{id}/dependents */
export function getDependents(version: Version, id: string): Promise<ApiEdge[]> {
  return fetchJson(`${BASE}/api/snapshots/${version}/nodes/${encodeURIComponent(id)}/dependents`);
}

/** GET /api/snapshots/{v}/edges */
export function getEdges(version: Version, kind?: string): Promise<ApiEdge[]> {
  const params = kind ? `?kind=${encodeURIComponent(kind)}` : "";
  return fetchJson(`${BASE}/api/snapshots/${version}/edges${params}`);
}

/** GET /api/snapshots/{v}/graph */
export function getGraph(version: Version): Promise<CytoscapeGraph> {
  return fetchJson(`${BASE}/api/snapshots/${version}/graph`);
}

/** GET /api/conformance/design/{v} */
export function getDesignConformance(version: Version): Promise<ConformanceReport> {
  return fetchJson(`${BASE}/api/conformance/design/${version}`);
}

/** GET /api/conformance?design=V&analysis=V */
export function getConformance(design: Version, analysis: Version): Promise<ConformanceReport> {
  return fetchJson(`${BASE}/api/conformance?design=${design}&analysis=${analysis}`);
}

/** GET /api/search?path=GLOB&version=V */
export function searchNodes(path: string, version: Version): Promise<ApiNode[]> {
  return fetchJson(`${BASE}/api/search?path=${encodeURIComponent(path)}&version=${version}`);
}
```

**Step 3: Create unit test for api.ts**

```typescript
// web/src/lib/__tests__/api.test.ts
import { describe, it, expect, vi, beforeEach } from "vitest";
import { getSnapshots, getGraph, getHealth, searchNodes } from "../api";

// Mock fetch globally
const mockFetch = vi.fn();
vi.stubGlobal("fetch", mockFetch);

beforeEach(() => {
  mockFetch.mockReset();
});

function mockResponse(data: unknown, ok = true, status = 200) {
  return {
    ok,
    status,
    statusText: "OK",
    json: () => Promise.resolve(data),
  };
}

describe("api", () => {
  it("getHealth fetches /api/health", async () => {
    mockFetch.mockResolvedValueOnce(mockResponse({ status: "ok" }));
    const result = await getHealth();
    expect(result).toEqual({ status: "ok" });
    expect(mockFetch).toHaveBeenCalledWith("/api/health");
  });

  it("getSnapshots fetches /api/snapshots", async () => {
    const data = [{ version: 1, kind: "design", commit_ref: null }];
    mockFetch.mockResolvedValueOnce(mockResponse(data));
    const result = await getSnapshots();
    expect(result).toEqual(data);
    expect(mockFetch).toHaveBeenCalledWith("/api/snapshots");
  });

  it("getGraph fetches /api/snapshots/{v}/graph", async () => {
    const data = { elements: { nodes: [], edges: [] } };
    mockFetch.mockResolvedValueOnce(mockResponse(data));
    const result = await getGraph(1);
    expect(result).toEqual(data);
    expect(mockFetch).toHaveBeenCalledWith("/api/snapshots/1/graph");
  });

  it("searchNodes encodes path parameter", async () => {
    mockFetch.mockResolvedValueOnce(mockResponse([]));
    await searchNodes("/svt/**", 1);
    expect(mockFetch).toHaveBeenCalledWith("/api/search?path=%2Fsvt%2F**&version=1");
  });

  it("throws on HTTP error with server message", async () => {
    mockFetch.mockResolvedValueOnce(
      mockResponse({ error: "not found" }, false, 404),
    );
    await expect(getHealth()).rejects.toThrow("not found");
  });
});
```

**Step 4: Run tests**

Run from `web/`:
```bash
npm test
```
Expected: 5 tests pass.

**Step 5: Commit**

```bash
git add web/src/lib/
git commit -m "feat(web): add TypeScript types and typed API client with tests"
```

---

### Task 2: Reactive Stores

**Files:**
- Create: `web/src/stores/graph.ts`
- Create: `web/src/stores/selection.ts`

Svelte 5 uses runes (`$state`, `$derived`) for reactivity. These stores hold the shared application state.

**Step 1: Create graph.ts store**

```typescript
import type { Snapshot, CytoscapeGraph, ConformanceReport, Version } from "../lib/types";

/** Reactive store for graph data and snapshot state. */
class GraphStore {
  snapshots = $state<Snapshot[]>([]);
  selectedVersion = $state<Version | null>(null);
  graph = $state<CytoscapeGraph | null>(null);
  conformanceReport = $state<ConformanceReport | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);

  /** Design snapshots only. */
  get designSnapshots(): Snapshot[] {
    return this.snapshots.filter((s) => s.kind === "design");
  }

  /** Analysis snapshots only. */
  get analysisSnapshots(): Snapshot[] {
    return this.snapshots.filter((s) => s.kind === "analysis");
  }

  /** Clear error state. */
  clearError() {
    this.error = null;
  }
}

export const graphStore = new GraphStore();
```

**Step 2: Create selection.ts store**

```typescript
import type { ApiNode, ApiEdge } from "../lib/types";

/** Reactive store for node selection and detail panel state. */
class SelectionStore {
  selectedNodeId = $state<string | null>(null);
  selectedNode = $state<ApiNode | null>(null);
  children = $state<ApiNode[]>([]);
  ancestors = $state<ApiNode[]>([]);
  dependencies = $state<ApiEdge[]>([]);
  dependents = $state<ApiEdge[]>([]);
  panelOpen = $state(false);
  loading = $state(false);

  /** Clear selection and close panel. */
  clear() {
    this.selectedNodeId = null;
    this.selectedNode = null;
    this.children = [];
    this.ancestors = [];
    this.dependencies = [];
    this.dependents = [];
    this.panelOpen = false;
  }
}

export const selectionStore = new SelectionStore();
```

**Step 3: Verify build**

Run from `web/`:
```bash
npm run build
```
Expected: success (stores are imported later by components).

**Step 4: Commit**

```bash
git add web/src/stores/
git commit -m "feat(web): add reactive graph and selection stores"
```

---

### Task 3: GraphView Component

**Files:**
- Create: `web/src/components/GraphView.svelte`

This is the core visual component. It initialises Cytoscape.js, renders the graph with compound nodes, handles click events, and applies conformance overlays.

**Step 1: Create GraphView.svelte**

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import cytoscape from "cytoscape";
  import coseBilkent from "cytoscape-cose-bilkent";
  import dagre from "cytoscape-dagre";
  import type { CytoscapeGraph, ConformanceReport } from "../lib/types";
  import { selectionStore } from "../stores/selection";

  // Register layout extensions once
  cytoscape.use(coseBilkent);
  cytoscape.use(dagre);

  interface Props {
    graph: CytoscapeGraph | null;
    conformance?: ConformanceReport | null;
    layout?: "cose-bilkent" | "dagre";
  }

  let { graph, conformance = null, layout = "cose-bilkent" }: Props = $props();

  let container: HTMLDivElement;
  let cy: cytoscape.Core | null = null;

  const styleSheet: cytoscape.Stylesheet[] = [
    {
      selector: "node",
      style: {
        label: "data(label)",
        "text-valign": "center",
        "text-halign": "center",
        "background-color": "#53a8b6",
        color: "#fff",
        "font-size": "12px",
        "text-wrap": "wrap",
        "text-max-width": "80px",
        width: "label",
        height: "label",
        padding: "10px",
        shape: "roundrectangle",
      },
    },
    {
      selector: "node:parent",
      style: {
        "background-color": "#16213e",
        "background-opacity": 0.6,
        "border-color": "#0f3460",
        "border-width": 2,
        "text-valign": "top",
        "text-halign": "center",
        "font-size": "14px",
        "font-weight": "bold",
        padding: "20px",
      },
    },
    {
      selector: "edge",
      style: {
        width: 2,
        "line-color": "#607d8b",
        "target-arrow-color": "#607d8b",
        "target-arrow-shape": "triangle",
        "curve-style": "bezier",
        "arrow-scale": 0.8,
      },
    },
    {
      selector: "edge[kind = 'depends']",
      style: { "line-style": "solid", "line-color": "#53a8b6", "target-arrow-color": "#53a8b6" },
    },
    {
      selector: "edge[kind = 'data_flow']",
      style: { "line-style": "dashed", "line-color": "#ff9800", "target-arrow-color": "#ff9800" },
    },
    {
      selector: "edge[kind = 'implements']",
      style: { "line-style": "dotted", "line-color": "#4caf50", "target-arrow-color": "#4caf50" },
    },
    {
      selector: "node:selected",
      style: { "border-color": "#fff", "border-width": 3 },
    },
    // Conformance overlay classes
    {
      selector: ".conformance-pass",
      style: { "border-color": "#4caf50", "border-width": 3 },
    },
    {
      selector: ".conformance-fail",
      style: { "border-color": "#f44336", "border-width": 3 },
    },
    {
      selector: ".conformance-unimplemented",
      style: { "border-color": "#ff9800", "border-width": 3 },
    },
    {
      selector: ".conformance-undocumented",
      style: { "border-color": "#607d8b", "border-width": 3 },
    },
  ];

  function initCytoscape(elements: CytoscapeGraph["elements"]) {
    if (cy) cy.destroy();

    cy = cytoscape({
      container,
      elements: {
        nodes: elements.nodes,
        edges: elements.edges,
      },
      style: styleSheet,
      layout: {
        name: layout,
        animate: false,
        nodeDimensionsIncludeLabels: true,
      } as cytoscape.LayoutOptions,
    });

    cy.on("tap", "node", (evt) => {
      const nodeId = evt.target.id();
      selectionStore.selectedNodeId = nodeId;
      selectionStore.panelOpen = true;
    });

    cy.on("tap", (evt) => {
      if (evt.target === cy) {
        selectionStore.clear();
      }
    });
  }

  function applyConformanceOverlay(report: ConformanceReport) {
    if (!cy) return;

    // Clear previous overlay
    cy.nodes().removeClass(
      "conformance-pass conformance-fail conformance-unimplemented conformance-undocumented",
    );

    // Mark failed constraints
    for (const result of report.constraint_results) {
      if (result.status === "fail") {
        for (const violation of result.violations) {
          const node = cy.nodes().filter((n) => n.data("canonical_path") === violation.source_path);
          node.addClass("conformance-fail");
        }
      }
    }

    // Mark unimplemented
    for (const node of report.unimplemented) {
      cy.nodes()
        .filter((n) => n.data("canonical_path") === node.canonical_path)
        .addClass("conformance-unimplemented");
    }

    // Mark undocumented
    for (const node of report.undocumented) {
      cy.nodes()
        .filter((n) => n.data("canonical_path") === node.canonical_path)
        .addClass("conformance-undocumented");
    }

    // Remaining nodes with no overlay = pass
    cy.nodes()
      .filter(
        (n) =>
          !n.hasClass("conformance-fail") &&
          !n.hasClass("conformance-unimplemented") &&
          !n.hasClass("conformance-undocumented"),
      )
      .addClass("conformance-pass");
  }

  onMount(() => {
    return () => {
      if (cy) cy.destroy();
    };
  });

  $effect(() => {
    if (graph && container) {
      initCytoscape(graph.elements);
    }
  });

  $effect(() => {
    if (conformance && cy) {
      applyConformanceOverlay(conformance);
    }
  });

  /** Re-run layout. */
  export function relayout(name?: "cose-bilkent" | "dagre") {
    if (!cy) return;
    cy.layout({
      name: name || layout,
      animate: true,
      nodeDimensionsIncludeLabels: true,
    } as cytoscape.LayoutOptions).run();
  }
</script>

<div class="graph-container" bind:this={container}></div>

<style>
  .graph-container {
    flex: 1;
    min-height: 0;
    background: var(--bg);
  }
</style>
```

**Step 2: Verify build**

Run from `web/`:
```bash
npm run build
```
Expected: success. The component won't render yet (needs to be used in App.svelte).

**Step 3: Commit**

```bash
git add web/src/components/GraphView.svelte
git commit -m "feat(web): add GraphView component with Cytoscape.js rendering"
```

---

### Task 4: NodeDetail Component

**Files:**
- Create: `web/src/components/NodeDetail.svelte`

Side panel showing node metadata, ancestors, children, dependencies, dependents.

**Step 1: Create NodeDetail.svelte**

```svelte
<script lang="ts">
  import type { ApiNode, ApiEdge } from "../lib/types";
  import { selectionStore } from "../stores/selection";

  interface Props {
    node: ApiNode | null;
    children: ApiNode[];
    ancestors: ApiNode[];
    dependencies: ApiEdge[];
    dependents: ApiEdge[];
    loading: boolean;
  }

  let { node, children, ancestors, dependencies, dependents, loading }: Props = $props();

  function close() {
    selectionStore.clear();
  }
</script>

{#if node}
  <aside class="node-detail">
    <header>
      <h2>{node.name}</h2>
      <button onclick={close} aria-label="Close panel">&times;</button>
    </header>

    <section>
      <dl>
        <dt>Kind</dt>
        <dd>{node.kind} / {node.sub_kind}</dd>
        <dt>Path</dt>
        <dd><code>{node.canonical_path}</code></dd>
        {#if node.qualified_name}
          <dt>Qualified Name</dt>
          <dd><code>{node.qualified_name}</code></dd>
        {/if}
        {#if node.language}
          <dt>Language</dt>
          <dd>{node.language}</dd>
        {/if}
        {#if node.source_ref}
          <dt>Source</dt>
          <dd><code>{node.source_ref}</code></dd>
        {/if}
        <dt>Provenance</dt>
        <dd>{node.provenance}</dd>
      </dl>
    </section>

    {#if loading}
      <p class="loading">Loading details...</p>
    {:else}
      {#if ancestors.length > 0}
        <section>
          <h3>Ancestors ({ancestors.length})</h3>
          <ul>
            {#each ancestors as a}
              <li><code>{a.canonical_path}</code></li>
            {/each}
          </ul>
        </section>
      {/if}

      {#if children.length > 0}
        <section>
          <h3>Children ({children.length})</h3>
          <ul>
            {#each children as c}
              <li><code>{c.canonical_path}</code></li>
            {/each}
          </ul>
        </section>
      {/if}

      {#if dependencies.length > 0}
        <section>
          <h3>Dependencies ({dependencies.length})</h3>
          <ul>
            {#each dependencies as d}
              <li>{d.kind}: {d.target}</li>
            {/each}
          </ul>
        </section>
      {/if}

      {#if dependents.length > 0}
        <section>
          <h3>Dependents ({dependents.length})</h3>
          <ul>
            {#each dependents as d}
              <li>{d.kind}: {d.source}</li>
            {/each}
          </ul>
        </section>
      {/if}
    {/if}
  </aside>
{/if}

<style>
  .node-detail {
    width: 360px;
    background: var(--surface);
    border-left: 1px solid var(--border);
    overflow-y: auto;
    padding: 1rem;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  header h2 {
    font-size: 1.1rem;
    word-break: break-all;
  }

  header button {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 1.5rem;
    cursor: pointer;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.25rem 0.75rem;
  }

  dt {
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  dd {
    font-size: 0.85rem;
    word-break: break-all;
  }

  section {
    margin-bottom: 1rem;
  }

  h3 {
    font-size: 0.9rem;
    color: var(--text-muted);
    margin-bottom: 0.5rem;
  }

  ul {
    list-style: none;
    font-size: 0.85rem;
  }

  li {
    padding: 0.2rem 0;
    border-bottom: 1px solid var(--border);
  }

  code {
    font-size: 0.8rem;
    color: var(--accent);
  }

  .loading {
    color: var(--text-muted);
    font-style: italic;
  }
</style>
```

**Step 2: Verify build**

Run from `web/`:
```bash
npm run build
```
Expected: success.

**Step 3: Commit**

```bash
git add web/src/components/NodeDetail.svelte
git commit -m "feat(web): add NodeDetail side panel component"
```

---

### Task 5: SnapshotSelector and SearchBar Components

**Files:**
- Create: `web/src/components/SnapshotSelector.svelte`
- Create: `web/src/components/SearchBar.svelte`

**Step 1: Create SnapshotSelector.svelte**

```svelte
<script lang="ts">
  import type { Snapshot, Version } from "../lib/types";

  interface Props {
    snapshots: Snapshot[];
    selectedVersion: Version | null;
    onselect: (version: Version) => void;
  }

  let { snapshots, selectedVersion, onselect }: Props = $props();

  function handleChange(event: Event) {
    const target = event.target as HTMLSelectElement;
    const version = parseInt(target.value, 10);
    if (!isNaN(version)) {
      onselect(version);
    }
  }
</script>

<div class="snapshot-selector">
  <label for="snapshot-select">Snapshot:</label>
  <select id="snapshot-select" value={selectedVersion ?? ""} onchange={handleChange}>
    <option value="" disabled>Select a version...</option>
    {#each snapshots as snap}
      <option value={snap.version}>
        v{snap.version} ({snap.kind}{snap.commit_ref ? ` - ${snap.commit_ref}` : ""})
      </option>
    {/each}
  </select>
</div>

<style>
  .snapshot-selector {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  label {
    color: var(--text-muted);
    font-size: 0.85rem;
    white-space: nowrap;
  }

  select {
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--border);
    padding: 0.3rem 0.5rem;
    border-radius: 4px;
    font-size: 0.85rem;
  }
</style>
```

**Step 2: Create SearchBar.svelte**

```svelte
<script lang="ts">
  interface Props {
    onsearch: (query: string) => void;
  }

  let { onsearch }: Props = $props();
  let query = $state("");

  function handleSubmit(event: Event) {
    event.preventDefault();
    if (query.trim()) {
      onsearch(query.trim());
    }
  }
</script>

<form class="search-bar" onsubmit={handleSubmit}>
  <input
    type="text"
    placeholder="Search paths (e.g. /svt/core/**)"
    bind:value={query}
  />
  <button type="submit">Search</button>
</form>

<style>
  .search-bar {
    display: flex;
    gap: 0.5rem;
  }

  input {
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--border);
    padding: 0.3rem 0.5rem;
    border-radius: 4px;
    font-size: 0.85rem;
    flex: 1;
    min-width: 200px;
  }

  input::placeholder {
    color: var(--text-muted);
  }

  button {
    background: var(--accent);
    color: #fff;
    border: none;
    padding: 0.3rem 0.75rem;
    border-radius: 4px;
    font-size: 0.85rem;
    cursor: pointer;
  }

  button:hover {
    opacity: 0.9;
  }
</style>
```

**Step 3: Verify build**

Run from `web/`:
```bash
npm run build
```
Expected: success.

**Step 4: Commit**

```bash
git add web/src/components/SnapshotSelector.svelte web/src/components/SearchBar.svelte
git commit -m "feat(web): add SnapshotSelector and SearchBar components"
```

---

### Task 6: ConformanceReport Component

**Files:**
- Create: `web/src/components/ConformanceReport.svelte`

Displays conformance results: summary counts, constraint results, unimplemented/undocumented nodes.

**Step 1: Create ConformanceReport.svelte**

```svelte
<script lang="ts">
  import type { ConformanceReport } from "../lib/types";

  interface Props {
    report: ConformanceReport;
    onclose: () => void;
  }

  let { report, onclose }: Props = $props();
</script>

<aside class="conformance-panel">
  <header>
    <h2>Conformance Report</h2>
    <button onclick={onclose} aria-label="Close">&times;</button>
  </header>

  <section class="summary">
    <div class="stat pass">{report.summary.passed} passed</div>
    <div class="stat fail">{report.summary.failed} failed</div>
    <div class="stat warn">{report.summary.warned} warned</div>
    <div class="stat muted">{report.summary.not_evaluable} n/a</div>
    <div class="stat unimpl">{report.summary.unimplemented} unimplemented</div>
    <div class="stat undoc">{report.summary.undocumented} undocumented</div>
  </section>

  {#if report.constraint_results.length > 0}
    <section>
      <h3>Constraints</h3>
      {#each report.constraint_results as cr}
        <div class="constraint" class:fail={cr.status === "fail"} class:pass={cr.status === "pass"}>
          <span class="badge">{cr.status}</span>
          <strong>{cr.constraint_name}</strong>
          <p>{cr.message}</p>
          {#if cr.violations.length > 0}
            <ul>
              {#each cr.violations as v}
                <li>
                  <code>{v.source_path}</code>
                  {#if v.target_path} &rarr; <code>{v.target_path}</code>{/if}
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {/each}
    </section>
  {/if}

  {#if report.unimplemented.length > 0}
    <section>
      <h3>Unimplemented ({report.unimplemented.length})</h3>
      <ul>
        {#each report.unimplemented as n}
          <li><code>{n.canonical_path}</code> ({n.kind})</li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if report.undocumented.length > 0}
    <section>
      <h3>Undocumented ({report.undocumented.length})</h3>
      <ul>
        {#each report.undocumented as n}
          <li><code>{n.canonical_path}</code> ({n.kind})</li>
        {/each}
      </ul>
    </section>
  {/if}
</aside>

<style>
  .conformance-panel {
    width: 400px;
    background: var(--surface);
    border-left: 1px solid var(--border);
    overflow-y: auto;
    padding: 1rem;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  header button {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 1.5rem;
    cursor: pointer;
  }

  .summary {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.5rem;
    margin-bottom: 1rem;
  }

  .stat {
    padding: 0.5rem;
    border-radius: 4px;
    font-size: 0.85rem;
    text-align: center;
    background: var(--bg);
  }

  .stat.pass { border-left: 3px solid var(--pass); }
  .stat.fail { border-left: 3px solid var(--fail); }
  .stat.warn { border-left: 3px solid var(--warn); }
  .stat.muted { border-left: 3px solid var(--muted); }
  .stat.unimpl { border-left: 3px solid var(--warn); }
  .stat.undoc { border-left: 3px solid var(--muted); }

  .constraint {
    padding: 0.5rem;
    margin-bottom: 0.5rem;
    border-radius: 4px;
    background: var(--bg);
  }

  .constraint.fail { border-left: 3px solid var(--fail); }
  .constraint.pass { border-left: 3px solid var(--pass); }

  .badge {
    font-size: 0.75rem;
    text-transform: uppercase;
    padding: 0.1rem 0.3rem;
    border-radius: 2px;
    background: var(--surface);
  }

  h3 {
    font-size: 0.9rem;
    color: var(--text-muted);
    margin-bottom: 0.5rem;
  }

  section {
    margin-bottom: 1rem;
  }

  ul {
    list-style: none;
    font-size: 0.85rem;
  }

  li {
    padding: 0.2rem 0;
  }

  code {
    font-size: 0.8rem;
    color: var(--accent);
  }

  p {
    font-size: 0.85rem;
    color: var(--text-muted);
    margin: 0.25rem 0;
  }
</style>
```

**Step 2: Verify build**

Run from `web/`:
```bash
npm run build
```
Expected: success.

**Step 3: Commit**

```bash
git add web/src/components/ConformanceReport.svelte
git commit -m "feat(web): add ConformanceReport panel component"
```

---

### Task 7: App Layout and Integration

**Files:**
- Modify: `web/src/App.svelte`

Wire everything together: load snapshots on mount, handle version selection, fetch graph, handle node selection, conformance overlay.

**Step 1: Rewrite App.svelte**

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "./lib/api";
  import type { Version } from "./lib/types";
  import { graphStore } from "./stores/graph";
  import { selectionStore } from "./stores/selection";
  import GraphView from "./components/GraphView.svelte";
  import NodeDetail from "./components/NodeDetail.svelte";
  import ConformanceReport from "./components/ConformanceReport.svelte";
  import SnapshotSelector from "./components/SnapshotSelector.svelte";
  import SearchBar from "./components/SearchBar.svelte";

  let layoutChoice = $state<"cose-bilkent" | "dagre">("cose-bilkent");
  let graphView: GraphView;
  let showConformance = $state(false);
  let conformanceDesign = $state<Version | null>(null);
  let conformanceAnalysis = $state<Version | null>(null);

  onMount(async () => {
    try {
      graphStore.loading = true;
      graphStore.snapshots = await api.getSnapshots();
      if (graphStore.snapshots.length > 0) {
        await selectVersion(graphStore.snapshots[0].version);
      }
    } catch (e) {
      graphStore.error = e instanceof Error ? e.message : "Failed to load";
    } finally {
      graphStore.loading = false;
    }
  });

  async function selectVersion(version: Version) {
    try {
      graphStore.loading = true;
      graphStore.error = null;
      graphStore.selectedVersion = version;
      graphStore.graph = await api.getGraph(version);
      graphStore.conformanceReport = null;
      showConformance = false;
      selectionStore.clear();
    } catch (e) {
      graphStore.error = e instanceof Error ? e.message : "Failed to load graph";
    } finally {
      graphStore.loading = false;
    }
  }

  // React to node selection changes
  $effect(() => {
    const nodeId = selectionStore.selectedNodeId;
    const version = graphStore.selectedVersion;
    if (nodeId && version) {
      loadNodeDetails(version, nodeId);
    }
  });

  async function loadNodeDetails(version: Version, nodeId: string) {
    selectionStore.loading = true;
    try {
      const [node, children, ancestors, deps, dependents] = await Promise.all([
        api.getNode(version, nodeId),
        api.getChildren(version, nodeId),
        api.getAncestors(version, nodeId),
        api.getDependencies(version, nodeId),
        api.getDependents(version, nodeId),
      ]);
      selectionStore.selectedNode = node;
      selectionStore.children = children;
      selectionStore.ancestors = ancestors;
      selectionStore.dependencies = deps;
      selectionStore.dependents = dependents;
    } catch {
      // Node may not have all data — partial load is OK
    } finally {
      selectionStore.loading = false;
    }
  }

  async function handleSearch(query: string) {
    if (!graphStore.selectedVersion) return;
    try {
      const results = await api.searchNodes(query, graphStore.selectedVersion);
      if (results.length > 0) {
        selectionStore.selectedNodeId = results[0].id;
        selectionStore.panelOpen = true;
      }
    } catch (e) {
      graphStore.error = e instanceof Error ? e.message : "Search failed";
    }
  }

  async function loadConformance() {
    if (!conformanceDesign) return;
    try {
      graphStore.loading = true;
      if (conformanceAnalysis) {
        graphStore.conformanceReport = await api.getConformance(
          conformanceDesign,
          conformanceAnalysis,
        );
      } else {
        graphStore.conformanceReport = await api.getDesignConformance(conformanceDesign);
      }
      showConformance = true;
    } catch (e) {
      graphStore.error = e instanceof Error ? e.message : "Conformance failed";
    } finally {
      graphStore.loading = false;
    }
  }

  function clearConformance() {
    graphStore.conformanceReport = null;
    showConformance = false;
  }
</script>

<div class="app">
  <nav class="toolbar">
    <div class="toolbar-left">
      <span class="logo">SVT</span>
      <SnapshotSelector
        snapshots={graphStore.snapshots}
        selectedVersion={graphStore.selectedVersion}
        onselect={selectVersion}
      />
      <SearchBar onsearch={handleSearch} />
    </div>
    <div class="toolbar-right">
      <select bind:value={layoutChoice} onchange={() => graphView?.relayout(layoutChoice)}>
        <option value="cose-bilkent">Force-directed</option>
        <option value="dagre">Hierarchical</option>
      </select>

      {#if graphStore.designSnapshots.length > 0}
        <select bind:value={conformanceDesign}>
          <option value={null}>Design...</option>
          {#each graphStore.designSnapshots as s}
            <option value={s.version}>Design v{s.version}</option>
          {/each}
        </select>
      {/if}

      {#if graphStore.analysisSnapshots.length > 0}
        <select bind:value={conformanceAnalysis}>
          <option value={null}>Analysis...</option>
          {#each graphStore.analysisSnapshots as s}
            <option value={s.version}>Analysis v{s.version}</option>
          {/each}
        </select>
      {/if}

      <button onclick={loadConformance} disabled={!conformanceDesign}>
        Check Conformance
      </button>
    </div>
  </nav>

  {#if graphStore.error}
    <div class="error-bar">
      {graphStore.error}
      <button onclick={() => graphStore.clearError()}>Dismiss</button>
    </div>
  {/if}

  <div class="main-content">
    {#if graphStore.loading && !graphStore.graph}
      <div class="center-message">Loading...</div>
    {:else if graphStore.graph}
      <GraphView
        bind:this={graphView}
        graph={graphStore.graph}
        conformance={graphStore.conformanceReport}
        layout={layoutChoice}
      />
    {:else}
      <div class="center-message">No data loaded. Start the server with --design or --project.</div>
    {/if}

    {#if selectionStore.panelOpen}
      <NodeDetail
        node={selectionStore.selectedNode}
        children={selectionStore.children}
        ancestors={selectionStore.ancestors}
        dependencies={selectionStore.dependencies}
        dependents={selectionStore.dependents}
        loading={selectionStore.loading}
      />
    {/if}

    {#if showConformance && graphStore.conformanceReport}
      <ConformanceReport report={graphStore.conformanceReport} onclose={clearConformance} />
    {/if}
  </div>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 1rem;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    gap: 1rem;
    flex-wrap: wrap;
  }

  .toolbar-left,
  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .logo {
    font-weight: bold;
    font-size: 1.1rem;
    color: var(--accent);
    margin-right: 0.5rem;
  }

  select,
  button {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    padding: 0.3rem 0.5rem;
    border-radius: 4px;
    font-size: 0.85rem;
  }

  button {
    cursor: pointer;
    background: var(--accent);
    color: #fff;
    border: none;
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .error-bar {
    background: var(--fail);
    color: #fff;
    padding: 0.5rem 1rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .error-bar button {
    background: rgba(255, 255, 255, 0.2);
    font-size: 0.8rem;
  }

  .main-content {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  .center-message {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: 1.2rem;
  }
</style>
```

**Step 2: Verify build**

Run from `web/`:
```bash
npm run build
```
Expected: success. Produces `web/dist/` with the full app.

**Step 3: Commit**

```bash
git add web/src/App.svelte
git commit -m "feat(web): integrate all components into App layout"
```

---

### Task 8: Server Static File Serving

**Files:**
- Modify: `crates/server/src/routes/mod.rs`
- Modify: `crates/server/src/main.rs`

Add `tower-http::ServeDir` to serve `web/dist/` at `/` in production (when the directory exists).

**Step 1: Update routes/mod.rs**

Add a `pub fn full_router` function that wraps `api_router` with static file serving:

After the existing `api_router` function, add:

```rust
/// Build the full router with API routes and optional static file serving.
///
/// If `static_dir` is provided and the directory exists, serves static files at `/`.
/// API routes take priority over static files.
pub fn full_router(state: Arc<AppState>, static_dir: Option<std::path::PathBuf>) -> Router {
    let router = api_router(state);

    if let Some(dir) = static_dir {
        if dir.exists() {
            tracing::info!(path = %dir.display(), "serving static files");
            return router.fallback_service(
                tower_http::services::ServeDir::new(dir)
                    .fallback(tower_http::services::ServeFile::new("web/dist/index.html")),
            );
        }
    }

    router
}
```

Add the import at the top of `routes/mod.rs`:
```rust
use tracing;
```

**Step 2: Update main.rs to use full_router**

In `main.rs`, change:
```rust
let app = routes::api_router(state);
```
to:
```rust
let static_dir = std::path::PathBuf::from("web/dist");
let app = routes::full_router(state, Some(static_dir));
```

**Step 3: Run tests**

Run: `cargo test -p svt-server`
Expected: all 17 tests pass (static serving doesn't affect API tests).

**Step 4: Commit**

```bash
git add crates/server/src/routes/mod.rs crates/server/src/main.rs
git commit -m "feat(server): serve web/dist static files via fallback route"
```

---

### Task 9: Verification and Dog-Food

**Files:**
- No new files. Manual testing and verification.

**Step 1: Build the frontend**

Run from `web/`:
```bash
npm run build
```
Expected: `web/dist/` populated.

**Step 2: Run all Rust tests**

```bash
cargo test --workspace
```
Expected: 218+ tests pass.

**Step 3: Run all frontend tests**

```bash
cd web && npm test
```
Expected: 5+ tests pass.

**Step 4: Run type check**

```bash
cd web && npm run check
```
Expected: clean.

**Step 5: Run clippy and fmt**

```bash
cargo clippy --workspace && cargo fmt --check
```
Expected: clean.

**Step 6: Manual dog-food test**

Terminal 1:
```bash
cargo run -p svt-server -- --project . --design design/architecture.yaml
```

Terminal 2 — verify API:
```bash
curl -s http://localhost:3000/api/health | jq .
curl -s http://localhost:3000/api/snapshots | jq .
curl -s http://localhost:3000/api/snapshots/1/graph | jq '.elements.nodes | length'
```

Then open `http://localhost:3000` in browser — should show the Svelte UI with the architecture graph.

**Step 7: Commit if any adjustments needed**

```bash
git log --oneline -12
```
