# Web UI Diff View + SVG/PNG Export Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add diff overlay visualization to the web UI and SVG/PNG export via Graphviz CLI piping.

**Architecture:** The diff view adds a "Compare to..." dropdown that triggers a diff API call and applies CSS classes to Cytoscape elements (same pattern as conformance overlay). SVG/PNG export adds two new `ExportFormat` implementations that pipe DOT output through the system `dot` command.

**Tech Stack:** Svelte 5 (runes), Cytoscape.js, vitest (web); Rust, `std::process::Command`, `ExportFormat` trait (backend)

---

### Task 1: SVG Exporter

**Files:**
- Create: `crates/core/src/export/svg.rs`
- Modify: `crates/core/src/export/mod.rs`

**Step 1: Write the failing test**

Add to `crates/core/src/export/mod.rs` in the `#[cfg(test)] mod tests` block:

```rust
#[test]
fn export_registry_with_defaults_includes_svg() {
    let registry = ExportRegistry::with_defaults();
    assert!(registry.get("svg").is_some(), "svg format should be registered");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-core export_registry_with_defaults_includes_svg`
Expected: FAIL — `svg` is not registered yet.

**Step 3: Create `crates/core/src/export/svg.rs`**

```rust
//! SVG export via Graphviz `dot` command.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::model::Version;
use crate::store::{GraphStore, Result, StoreError};

/// Generate SVG output by piping DOT through Graphviz `dot -Tsvg`.
///
/// Requires the `dot` command to be installed on the system (from Graphviz).
pub fn to_svg(store: &dyn GraphStore, version: Version) -> Result<String> {
    let dot_source = super::dot::to_dot(store, version)?;
    pipe_through_dot(&dot_source, "svg")
}

/// Pipe DOT source through `dot -T<format>` and return the output as a string.
pub(crate) fn pipe_through_dot(dot_source: &str, format: &str) -> Result<String> {
    let mut child = Command::new("dot")
        .arg(format!("-T{format}"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StoreError::Other(
                    "Graphviz `dot` command not found. \
                     Install from https://graphviz.org/"
                        .to_string(),
                )
            } else {
                StoreError::Other(format!("failed to run `dot`: {e}"))
            }
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(dot_source.as_bytes())
            .map_err(|e| StoreError::Other(format!("failed to write to dot stdin: {e}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| StoreError::Other(format!("failed to read dot output: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StoreError::Other(format!(
            "dot command failed: {stderr}"
        )));
    }

    String::from_utf8(output.stdout)
        .map_err(|e| StoreError::Other(format!("dot produced invalid UTF-8: {e}")))
}
```

**Step 4: Wire up in `crates/core/src/export/mod.rs`**

Add `pub mod svg;` after `pub mod mermaid;`.

Add the `SvgExporter` struct:

```rust
/// SVG exporter via Graphviz `dot` command.
#[derive(Debug)]
pub struct SvgExporter;

impl ExportFormat for SvgExporter {
    fn name(&self) -> &str {
        "svg"
    }
    fn export(&self, store: &dyn GraphStore, version: Version) -> Result<String> {
        svg::to_svg(store, version)
    }
}
```

Register it in `with_defaults()`:

```rust
registry.register(Box::new(SvgExporter));
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p svt-core export_registry_with_defaults_includes_svg`
Expected: PASS

Also update the existing `export_registry_with_defaults_has_all_built_ins` test to include `"svg"` in the expected names list.

**Step 6: Commit**

```bash
git add crates/core/src/export/svg.rs crates/core/src/export/mod.rs
git commit -m "feat(core): add SVG export via Graphviz dot piping"
```

---

### Task 2: PNG Exporter

**Files:**
- Modify: `crates/core/src/export/svg.rs` (add `to_png` function)
- Modify: `crates/core/src/export/mod.rs`

**Step 1: Write the failing test**

Add to `crates/core/src/export/mod.rs` tests:

```rust
#[test]
fn export_registry_with_defaults_includes_png() {
    let registry = ExportRegistry::with_defaults();
    assert!(registry.get("png").is_some(), "png format should be registered");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-core export_registry_with_defaults_includes_png`
Expected: FAIL

**Step 3: Add `to_png` function to `crates/core/src/export/svg.rs`**

```rust
/// Generate PNG output by piping DOT through Graphviz `dot -Tpng`.
///
/// Returns the PNG binary data as raw bytes.
pub fn to_png_bytes(store: &dyn GraphStore, version: Version) -> Result<Vec<u8>> {
    let dot_source = super::dot::to_dot(store, version)?;
    pipe_through_dot_bytes(&dot_source, "png")
}

/// Pipe DOT source through `dot -T<format>` and return raw bytes.
pub(crate) fn pipe_through_dot_bytes(dot_source: &str, format: &str) -> Result<Vec<u8>> {
    let mut child = Command::new("dot")
        .arg(format!("-T{format}"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StoreError::Other(
                    "Graphviz `dot` command not found. \
                     Install from https://graphviz.org/"
                        .to_string(),
                )
            } else {
                StoreError::Other(format!("failed to run `dot`: {e}"))
            }
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(dot_source.as_bytes())
            .map_err(|e| StoreError::Other(format!("failed to write to dot stdin: {e}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| StoreError::Other(format!("failed to read dot output: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StoreError::Other(format!(
            "dot command failed: {stderr}"
        )));
    }

    Ok(output.stdout)
}
```

**Step 4: Add `PngExporter` to `crates/core/src/export/mod.rs`**

The PNG exporter needs special handling since `ExportFormat::export` returns `String`, but PNG is binary. For the CLI, we need to handle binary output. The simplest approach: `PngExporter` writes directly to a file, and the `ExportFormat` trait returns a placeholder message when no output file is specified.

Actually, the cleaner approach is to make the CLI handle PNG specially. But to keep things simple and consistent with the registry pattern, we'll have the PNG exporter return base64-encoded data and have the CLI detect PNG format to write raw bytes.

Add a new trait method with a default implementation to `ExportFormat`:

No — YAGNI. Instead, handle it in the CLI: when format is "png", call `svg::to_png_bytes` directly and write binary. The `PngExporter` in the registry is just for discovery/validation.

```rust
/// PNG exporter via Graphviz `dot` command.
///
/// Note: PNG is binary. Use [`svg::to_png_bytes`] for raw binary output.
/// The `export()` method returns a message directing to use `--output` flag.
#[derive(Debug)]
pub struct PngExporter;

impl ExportFormat for PngExporter {
    fn name(&self) -> &str {
        "png"
    }
    fn export(&self, _store: &dyn GraphStore, _version: Version) -> Result<String> {
        Err(StoreError::Other(
            "PNG is a binary format. Use `svt export --format png --output FILE`".to_string(),
        ))
    }
}
```

Register it in `with_defaults()`:

```rust
registry.register(Box::new(PngExporter));
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p svt-core export_registry_with_defaults_includes_png`
Expected: PASS

Update `export_registry_with_defaults_has_all_built_ins` to include `"png"` and `"svg"` in expected names.

**Step 6: Commit**

```bash
git add crates/core/src/export/svg.rs crates/core/src/export/mod.rs
git commit -m "feat(core): add PNG export via Graphviz dot piping"
```

---

### Task 3: CLI PNG Export Handling

**Files:**
- Modify: `crates/cli/src/main.rs`

**Step 1: Update `run_export` to handle PNG binary output**

In `run_export()`, after getting the exporter, add special handling for PNG format:

```rust
// PNG requires an output file (binary format)
if args.format == "png" {
    let output_path = args.output.as_ref().ok_or_else(|| {
        anyhow::anyhow!("PNG is a binary format. Please specify --output FILE")
    })?;
    let png_bytes = svt_core::export::svg::to_png_bytes(&store, version)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    std::fs::write(output_path, &png_bytes)
        .with_context(|| format!("writing to {}", output_path.display()))?;
    println!("Exported PNG to {}", output_path.display());
    return Ok(());
}
```

Add this block before the existing `exporter.export()` call.

**Step 2: Run the test suite**

Run: `cargo test -p svt-cli`
Expected: All existing CLI tests pass.

**Step 3: Commit**

```bash
git add crates/cli/src/main.rs
git commit -m "feat(cli): handle PNG binary export with --output flag"
```

---

### Task 4: Add `diff` Parameter to Router

**Files:**
- Modify: `web/src/lib/router.ts`
- Modify: `web/src/lib/__tests__/router.test.ts`

**Step 1: Write failing tests**

Add to `web/src/lib/__tests__/router.test.ts`:

```typescript
it("parses diff parameter", () => {
  expect(parseHash("#v=2&diff=1")).toEqual({ version: 2, diff: 1 });
});
```

And in `buildHash` section:

```typescript
it("includes diff when present", () => {
  const hash = buildHash({ version: 2, diff: 1 });
  expect(hash).toContain("v=2");
  expect(hash).toContain("diff=1");
});

it("round-trips diff parameter", () => {
  const state = { version: 3, diff: 1, layout: "dagre" };
  expect(parseHash(buildHash(state))).toEqual(state);
});
```

**Step 2: Run tests to verify they fail**

Run: `cd web && npx vitest run`
Expected: FAIL — `diff` not in HashState

**Step 3: Update `web/src/lib/router.ts`**

Add `diff?: number;` to `HashState` interface.

In `parseHash`, add after the layout parsing:

```typescript
const diff = params.get("diff");
if (diff) state.diff = parseInt(diff, 10);
```

In `buildHash`, add after the layout line:

```typescript
if (state.diff !== undefined) params.set("diff", String(state.diff));
```

**Step 4: Run tests to verify they pass**

Run: `cd web && npx vitest run`
Expected: PASS (all 19+ tests including new ones)

**Step 5: Commit**

```bash
git add web/src/lib/router.ts web/src/lib/__tests__/router.test.ts
git commit -m "feat(web): add diff parameter to URL hash router"
```

---

### Task 5: Add Diff State to Graph Store

**Files:**
- Modify: `web/src/stores/graph.ts`

**Step 1: Add diff state fields to `GraphStore`**

```typescript
import type { Snapshot, CytoscapeGraph, ConformanceReport, SnapshotDiff, Version } from "../lib/types";

class GraphStore {
  // ... existing fields ...
  diffReport = $state<SnapshotDiff | null>(null);
  diffVersion = $state<Version | null>(null);

  // ... existing methods ...

  /** Clear diff state. */
  clearDiff() {
    this.diffReport = null;
    this.diffVersion = null;
  }
}
```

**Step 2: Run tests to verify nothing broke**

Run: `cd web && npx vitest run`
Expected: PASS

**Step 3: Commit**

```bash
git add web/src/stores/graph.ts
git commit -m "feat(web): add diff state to graph store"
```

---

### Task 6: Add Diff CSS Classes to GraphView

**Files:**
- Modify: `web/src/components/GraphView.svelte`

**Step 1: Add diff prop and overlay styles**

In `Props` interface, add:

```typescript
diff?: SnapshotDiff | null;
```

Import the type:

```typescript
import type { CytoscapeGraph, ConformanceReport, SnapshotDiff } from "../lib/types";
```

Update the destructuring:

```typescript
let { graph, conformance = null, diff = null, layout = "cose-bilkent", theme = "dark" }: Props = $props();
```

**Step 2: Add diff styles to `buildStyleSheet()`**

Add these after the `.conformance-undocumented` entry:

```typescript
{
  selector: ".diff-added",
  style: { "border-color": pass, "border-width": 3, "border-style": "dashed" },
},
{
  selector: ".diff-removed",
  style: { "border-color": fail, "border-width": 3, "border-style": "dashed", opacity: 0.5 },
},
{
  selector: ".diff-changed",
  style: { "border-color": warn, "border-width": 3, "border-style": "dashed" },
},
{
  selector: "edge.diff-added",
  style: { "line-color": pass, "target-arrow-color": pass, "line-style": "dashed" },
},
{
  selector: "edge.diff-removed",
  style: { "line-color": fail, "target-arrow-color": fail, "line-style": "dashed", opacity: 0.5 },
},
```

**Step 3: Add `applyDiffOverlay` function**

Add after `applyConformanceOverlay`:

```typescript
function applyDiffOverlay(report: SnapshotDiff) {
  if (!cy) return;

  // Clear previous diff overlay
  cy.elements().removeClass("diff-added diff-removed diff-changed");

  // Apply node changes
  for (const change of report.node_changes) {
    const node = cy.nodes().filter((n) => n.data("canonical_path") === change.canonical_path);
    if (node.length > 0) {
      node.addClass(`diff-${change.change}`);
    }
  }

  // Apply edge changes
  for (const change of report.edge_changes) {
    const edge = cy.edges().filter((e) => {
      const srcNode = cy.getElementById(e.data("source"));
      const tgtNode = cy.getElementById(e.data("target"));
      return (
        srcNode.data("canonical_path") === change.source_path &&
        tgtNode.data("canonical_path") === change.target_path &&
        e.data("kind") === change.edge_kind
      );
    });
    if (edge.length > 0) {
      edge.addClass(`diff-${change.change}`);
    }
  }
}

function clearDiffOverlay() {
  if (!cy) return;
  cy.elements().removeClass("diff-added diff-removed diff-changed");
}
```

**Step 4: Add effect to react to diff changes**

Add after the existing conformance effect:

```typescript
$effect(() => {
  if (diff && cy) {
    applyDiffOverlay(diff);
  } else if (!diff && cy) {
    clearDiffOverlay();
  }
});
```

**Step 5: Run tests to verify nothing broke**

Run: `cd web && npx vitest run`
Expected: PASS

**Step 6: Commit**

```bash
git add web/src/components/GraphView.svelte
git commit -m "feat(web): add diff overlay CSS classes and apply logic to GraphView"
```

---

### Task 7: Add Compare-To Dropdown and Diff Summary to App

**Files:**
- Modify: `web/src/App.svelte`

**Step 1: Add diff state and comparison dropdown**

Add state variable after `wasmVersion`:

```typescript
let compareVersion = $state<Version | null>(null);
```

Import `SnapshotDiff`:

```typescript
import type { Version, SnapshotDiff } from "./lib/types";
```

**Step 2: Add `loadDiff` and `clearDiff` functions**

Add after `clearConformance`:

```typescript
async function loadDiff(diffVersion: Version) {
  if (!graphStore.selectedVersion) return;
  try {
    graphStore.loading = true;
    const diff = await api.getDiff(diffVersion, graphStore.selectedVersion);
    graphStore.diffReport = diff;
    graphStore.diffVersion = diffVersion;
  } catch (e) {
    graphStore.error = e instanceof Error ? e.message : "Diff failed";
  } finally {
    graphStore.loading = false;
  }
}

function clearDiff() {
  compareVersion = null;
  graphStore.clearDiff();
}
```

**Step 3: Add effect to react to compareVersion changes**

```typescript
$effect(() => {
  if (compareVersion && graphStore.selectedVersion) {
    loadDiff(compareVersion);
  } else if (!compareVersion) {
    graphStore.clearDiff();
  }
});
```

**Step 4: Clear diff when primary snapshot changes**

In `selectVersion`, add after `showConformance = false;`:

```typescript
compareVersion = null;
graphStore.clearDiff();
```

**Step 5: Add the comparison dropdown to toolbar-left**

After `<SearchBar onsearch={handleSearch} />`, add:

```svelte
{#if graphStore.snapshots.length > 1 && graphStore.selectedVersion}
  <select
    bind:value={compareVersion}
    aria-label="Compare to version"
  >
    <option value={null}>Compare to...</option>
    {#each graphStore.snapshots.filter(s => s.version !== graphStore.selectedVersion) as s}
      <option value={s.version}>
        v{s.version} ({s.kind}{s.commit_ref ? ` - ${s.commit_ref}` : ""})
      </option>
    {/each}
  </select>
  {#if compareVersion}
    <button onclick={clearDiff} class="clear-btn">Clear diff</button>
  {/if}
{/if}
```

**Step 6: Add diff summary banner**

After the error bar `{/if}`, add:

```svelte
{#if graphStore.diffReport}
  <div class="diff-bar">
    Diff: v{graphStore.diffReport.from_version} → v{graphStore.diffReport.to_version}
    &nbsp;|&nbsp;
    <span class="diff-added-count">+{graphStore.diffReport.summary.nodes_added}</span>
    <span class="diff-removed-count">-{graphStore.diffReport.summary.nodes_removed}</span>
    <span class="diff-changed-count">~{graphStore.diffReport.summary.nodes_changed}</span>
    nodes
  </div>
{/if}
```

**Step 7: Pass diff to GraphView**

Update the `<GraphView>` component:

```svelte
<GraphView
  bind:this={graphView}
  graph={graphStore.graph}
  conformance={graphStore.conformanceReport}
  diff={graphStore.diffReport}
  layout={layoutChoice}
  {theme}
/>
```

**Step 8: Add diff bar CSS**

In `<style>`, add:

```css
.diff-bar {
  background: var(--surface);
  border-bottom: 1px solid var(--border);
  padding: 0.3rem 1rem;
  font-size: 0.85rem;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.diff-added-count { color: var(--pass); font-weight: bold; }
.diff-removed-count { color: var(--fail); font-weight: bold; }
.diff-changed-count { color: var(--warn); font-weight: bold; }

.clear-btn {
  background: var(--bg);
  color: var(--text);
  border: 1px solid var(--border);
  font-size: 0.75rem;
  padding: 0.15rem 0.4rem;
}
```

**Step 9: Integrate diff into URL hash**

Update the hash-writing effect to include diff:

```typescript
const hash = buildHash({
  version: graphStore.selectedVersion ?? undefined,
  node: selectionStore.selectedNodeId ?? undefined,
  layout: layoutChoice,
  diff: graphStore.diffVersion ?? undefined,
});
```

Update `onHashChange` to restore diff from hash:

```typescript
if (state.diff && state.diff !== graphStore.diffVersion) {
  compareVersion = state.diff;
}
```

**Step 10: Run tests**

Run: `cd web && npx vitest run`
Expected: PASS

**Step 11: Commit**

```bash
git add web/src/App.svelte
git commit -m "feat(web): add diff comparison dropdown, summary banner, and URL integration"
```

---

### Task 8: Update PROGRESS.md and Final Verification

**Files:**
- Modify: `docs/plan/PROGRESS.md`

**Step 1: Run all tests**

```bash
cargo test --workspace
cd web && npx vitest run
```

Expected: All tests pass.

**Step 2: Run quality checks**

```bash
cargo clippy --workspace -- -D warnings
cargo fmt -- --check
```

Expected: Clean.

**Step 3: Update PROGRESS.md**

Update the "Known Gaps" section:
- Mark "Web UI" gap's diff view as resolved
- Mark "Export Formats" gap as partially resolved (SVG/PNG added)
- Update test counts

Add a milestone summary entry for this work.

**Step 4: Commit**

```bash
git add docs/plan/PROGRESS.md
git commit -m "docs: update PROGRESS.md with diff view and SVG/PNG export"
```
