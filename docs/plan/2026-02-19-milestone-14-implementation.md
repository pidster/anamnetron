# Web UI Polish Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Improve the Svelte web frontend with dark/light theme toggle, hash-based URL routing, localStorage persistence, keyboard navigation, and better loading/empty/error states.

**Architecture:** All features are frontend-only — no server or Rust changes. Theme uses CSS custom properties with a `[data-theme]` attribute on `<html>`. URL routing uses `hashchange` events to sync state (selected node, layout, version) with the URL hash. localStorage saves layout preference and last-selected version. Keyboard navigation hooks into Cytoscape's API and global `keydown` events.

**Tech Stack:** Svelte 5 (runes), TypeScript, Cytoscape.js, CSS custom properties, browser `hashchange`/`localStorage`/`keydown` APIs. No new dependencies.

---

### Task 1: Light theme CSS variables and theme toggle

**Files:**
- Modify: `web/src/app.css` (add `[data-theme="light"]` variables)
- Modify: `web/src/App.svelte` (add theme toggle button in toolbar)
- Modify: `web/src/components/GraphView.svelte` (use CSS variables for Cytoscape colors)

**Step 1: Add light theme variables to app.css**

Add a `[data-theme="light"]` selector block after the `:root` block in `web/src/app.css`:

```css
[data-theme="light"] {
  --bg: #f5f5f5;
  --surface: #ffffff;
  --border: #d0d7de;
  --text: #1f2328;
  --text-muted: #656d76;
  --accent: #0969da;
  --pass: #1a7f37;
  --fail: #cf222e;
  --warn: #bf8700;
  --muted: #6e7781;
}
```

**Step 2: Add theme toggle to App.svelte toolbar**

In `web/src/App.svelte`:
- Add a `theme` state variable: `let theme = $state<"dark" | "light">("dark");`
- Add a toggle function that sets `document.documentElement.dataset.theme` and updates state
- Add a button in `.toolbar-left` after the logo: `<button onclick={toggleTheme}>{theme === "dark" ? "Light" : "Dark"}</button>`
- On mount, read `localStorage.getItem("svt-theme")` to restore preference
- In toggle function, write `localStorage.setItem("svt-theme", theme)`

**Step 3: Update GraphView to use theme-aware colors**

In `web/src/components/GraphView.svelte`, the Cytoscape stylesheet currently hardcodes hex colors (e.g., `#53a8b6`, `#16213e`, `#607d8b`). Since Cytoscape doesn't read CSS variables, add a reactive effect that reads the current computed CSS variable values and re-applies the stylesheet when theme changes.

Add a `theme` prop to GraphView. When theme changes, re-initialize Cytoscape with updated colors by reading `getComputedStyle(document.documentElement).getPropertyValue('--accent')` etc.

**Step 4: Run tests and verify**

Run: `cd web && npm test`
Expected: 5 tests pass (no regressions)

**Step 5: Commit**

```bash
git add web/src/app.css web/src/App.svelte web/src/components/GraphView.svelte
git commit -m "feat(web): add light/dark theme toggle with localStorage persistence"
```

---

### Task 2: Hash-based URL routing

**Files:**
- Create: `web/src/lib/router.ts`
- Modify: `web/src/App.svelte` (sync state with hash)
- Test: `web/src/lib/__tests__/router.test.ts`

**Step 1: Write router utility tests**

Create `web/src/lib/__tests__/router.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { parseHash, buildHash } from "../router";

describe("parseHash", () => {
  it("returns empty state for empty hash", () => {
    expect(parseHash("")).toEqual({});
  });

  it("parses version", () => {
    expect(parseHash("#v=1")).toEqual({ version: 1 });
  });

  it("parses version and node", () => {
    expect(parseHash("#v=1&node=abc")).toEqual({ version: 1, node: "abc" });
  });

  it("parses layout", () => {
    expect(parseHash("#v=1&layout=dagre")).toEqual({ version: 1, layout: "dagre" });
  });

  it("decodes URI components", () => {
    expect(parseHash("#v=1&node=%2Fsvt%2Fcore")).toEqual({ version: 1, node: "/svt/core" });
  });
});

describe("buildHash", () => {
  it("builds hash from state", () => {
    expect(buildHash({ version: 1 })).toBe("#v=1");
  });

  it("includes node when present", () => {
    const hash = buildHash({ version: 1, node: "abc" });
    expect(hash).toContain("v=1");
    expect(hash).toContain("node=abc");
  });

  it("omits undefined values", () => {
    expect(buildHash({ version: 1, node: undefined })).toBe("#v=1");
  });
});
```

**Step 2: Run tests to verify they fail**

Run: `cd web && npm test`
Expected: FAIL — `../router` module not found

**Step 3: Implement router utility**

Create `web/src/lib/router.ts`:

```typescript
/** State that can be encoded in the URL hash. */
export interface HashState {
  version?: number;
  node?: string;
  layout?: string;
}

/** Parse the URL hash into state. */
export function parseHash(hash: string): HashState {
  const clean = hash.replace(/^#/, "");
  if (!clean) return {};

  const params = new URLSearchParams(clean);
  const state: HashState = {};

  const v = params.get("v");
  if (v) state.version = parseInt(v, 10);

  const node = params.get("node");
  if (node) state.node = node;

  const layout = params.get("layout");
  if (layout) state.layout = layout;

  return state;
}

/** Build a URL hash string from state. */
export function buildHash(state: HashState): string {
  const params = new URLSearchParams();
  if (state.version !== undefined) params.set("v", String(state.version));
  if (state.node !== undefined) params.set("node", state.node);
  if (state.layout !== undefined) params.set("layout", state.layout);
  const str = params.toString();
  return str ? `#${str}` : "";
}
```

**Step 4: Run tests to verify they pass**

Run: `cd web && npm test`
Expected: All tests pass

**Step 5: Integrate router into App.svelte**

In `web/src/App.svelte`:
- Import `parseHash` and `buildHash` from `./lib/router`
- On mount, read `window.location.hash` and apply initial state (version, node, layout)
- Add a `$effect` that writes current state to `window.location.hash` using `buildHash` (but only when state changes, not during initial load)
- Add a `hashchange` event listener that reads and applies hash state (for back/forward navigation)
- Flag to suppress writing hash during reads: `let suppressHashWrite = false;`

**Step 6: Run tests and verify**

Run: `cd web && npm test`
Expected: All tests pass (no regressions)

**Step 7: Commit**

```bash
git add web/src/lib/router.ts web/src/lib/__tests__/router.test.ts web/src/App.svelte
git commit -m "feat(web): add hash-based URL routing for version, node, and layout"
```

---

### Task 3: localStorage persistence for layout preference

**Files:**
- Modify: `web/src/App.svelte` (persist layout choice)

**Step 1: Add localStorage read on mount**

In `web/src/App.svelte`, in the `onMount` callback:
- Read `localStorage.getItem("svt-layout")` and if valid ("cose-bilkent" or "dagre"), use it as initial `layoutChoice`
- The hash state takes priority over localStorage (hash is checked first in Task 2)

**Step 2: Add localStorage write on layout change**

Add a `$effect` that watches `layoutChoice` and writes to `localStorage.setItem("svt-layout", layoutChoice)`.

**Step 3: Run tests and verify**

Run: `cd web && npm test`
Expected: All tests pass

**Step 4: Commit**

```bash
git add web/src/App.svelte
git commit -m "feat(web): persist layout preference in localStorage"
```

---

### Task 4: Keyboard navigation

**Files:**
- Modify: `web/src/App.svelte` (global keydown handler)
- Modify: `web/src/components/GraphView.svelte` (expose navigation methods)

**Step 1: Add navigation methods to GraphView**

In `web/src/components/GraphView.svelte`, add exported functions:

```typescript
/** Select and center on a node. */
export function selectAndCenter(nodeId: string) {
  if (!cy) return;
  const node = cy.getElementById(nodeId);
  if (node.length === 0) return;
  cy.animate({ center: { eles: node }, duration: 200 });
  selectionStore.selectedNodeId = nodeId;
  selectionStore.panelOpen = true;
}

/** Fit all elements in viewport. */
export function fitAll() {
  if (!cy) return;
  cy.fit(undefined, 50);
}
```

**Step 2: Add global keydown handler in App.svelte**

In `web/src/App.svelte`:

```typescript
function handleKeydown(e: KeyboardEvent) {
  // Escape: close any open panel
  if (e.key === "Escape") {
    if (selectionStore.panelOpen) {
      selectionStore.clear();
      e.preventDefault();
    } else if (showConformance) {
      clearConformance();
      e.preventDefault();
    }
    return;
  }

  // Don't handle keys when focus is in an input
  if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;

  // f: fit all
  if (e.key === "f") {
    graphView?.fitAll();
    e.preventDefault();
  }
}
```

Attach to `<svelte:window onkeydown={handleKeydown} />` in the template.

**Step 3: Run tests and verify**

Run: `cd web && npm test`
Expected: All tests pass

**Step 4: Commit**

```bash
git add web/src/App.svelte web/src/components/GraphView.svelte
git commit -m "feat(web): add keyboard navigation (Escape to close, f to fit)"
```

---

### Task 5: Loading and empty states

**Files:**
- Modify: `web/src/App.svelte` (improve loading/empty/error UX)

**Step 1: Add a spinner/pulse animation for loading state**

In `web/src/App.svelte`, replace the simple "Loading..." text with:

```html
{#if graphStore.loading && !graphStore.graph}
  <div class="center-message">
    <div class="spinner"></div>
    <p>Loading graph data...</p>
  </div>
{:else if graphStore.graph}
  <!-- existing GraphView -->
{:else}
  <div class="center-message">
    <p>No data loaded</p>
    <p class="hint">Start the server with <code>--design</code> or <code>--project</code> flags.</p>
  </div>
{/if}
```

Add CSS for the spinner:

```css
.spinner {
  width: 32px;
  height: 32px;
  border: 3px solid var(--border);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  margin-bottom: 1rem;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.center-message p {
  margin: 0.25rem 0;
}

.hint {
  font-size: 0.9rem;
  color: var(--text-muted);
}

.hint code {
  color: var(--accent);
}
```

**Step 2: Run tests and verify**

Run: `cd web && npm test`
Expected: All tests pass

**Step 3: Commit**

```bash
git add web/src/App.svelte
git commit -m "feat(web): improve loading spinner and empty state messaging"
```

---

### Task 6: Add getDiff to API client and diff types

**Files:**
- Modify: `web/src/lib/api.ts` (add `getDiff` function)
- Modify: `web/src/lib/types.ts` (add diff-related types)
- Modify: `web/src/lib/__tests__/api.test.ts` (add getDiff test)

**Step 1: Add diff types to types.ts**

Add to the end of `web/src/lib/types.ts`:

```typescript
/** How a node or edge changed between snapshots. */
export type ChangeKind = "added" | "removed" | "changed";

/** A node that changed between two versions. */
export interface NodeChange {
  canonical_path: string;
  change: ChangeKind;
  kind: NodeKind;
  sub_kind: string;
  changed_fields: string[];
}

/** An edge that changed between two versions. */
export interface EdgeChange {
  source_path: string;
  target_path: string;
  edge_kind: EdgeKind;
  change: ChangeKind;
}

/** Summary counts for a snapshot diff. */
export interface DiffSummary {
  nodes_added: number;
  nodes_removed: number;
  nodes_changed: number;
  edges_added: number;
  edges_removed: number;
}

/** GET /api/diff response. */
export interface SnapshotDiff {
  from_version: Version;
  to_version: Version;
  node_changes: NodeChange[];
  edge_changes: EdgeChange[];
  summary: DiffSummary;
}
```

**Step 2: Add getDiff function to api.ts**

```typescript
/** GET /api/diff?from=V1&to=V2 */
export function getDiff(from: Version, to: Version): Promise<SnapshotDiff> {
  return fetchJson(`${BASE}/api/diff?from=${from}&to=${to}`);
}
```

**Step 3: Add test for getDiff**

Add to `web/src/lib/__tests__/api.test.ts`:

```typescript
it("getDiff fetches correct endpoint", async () => {
  mockFetch.mockResolvedValueOnce(
    mockResponse({ from_version: 1, to_version: 2, node_changes: [], edge_changes: [], summary: {} }),
  );
  await api.getDiff(1, 2);
  expect(mockFetch).toHaveBeenCalledWith("/api/diff?from=1&to=2");
});
```

**Step 4: Run tests**

Run: `cd web && npm test`
Expected: All tests pass

**Step 5: Commit**

```bash
git add web/src/lib/types.ts web/src/lib/api.ts web/src/lib/__tests__/api.test.ts
git commit -m "feat(web): add diff types and getDiff API client function"
```

---

### Task 7: Full verification, PROGRESS.md update, commit and push

**Step 1: Run all verification**

```bash
cargo fmt --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo audit
wasm-pack build crates/wasm --target web
cd web && npm test
```

Expected: All pass, no regressions

**Step 2: Update PROGRESS.md**

- Add M14 row to completed milestones table
- Update current state test count
- Update Web UI known gap section
- Add plan document reference

**Step 3: Commit and push**

```bash
git add docs/plan/PROGRESS.md docs/plan/2026-02-19-milestone-14-implementation.md
git commit -m "docs: mark milestone 14 as complete"
git push
```
