<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "./lib/api";
  import type { Version } from "./lib/types";
  import { graphStore } from "./stores/graph.svelte";
  import { selectionStore } from "./stores/selection.svelte";
  import { initWasm, getWasmStore } from "./lib/wasm";
  import { parseHash, buildHash } from "./lib/router";
  import { buildTraversalIndex, getParent, getFirstChild, getNextSibling, getPrevSibling } from "./lib/traversal";
  import NodeDetail from "./components/NodeDetail.svelte";
  import ConformanceReport from "./components/ConformanceReport.svelte";
  import ErrorBoundary from "./components/ErrorBoundary.svelte";
  import SnapshotSelector from "./components/SnapshotSelector.svelte";
  import SearchBar from "./components/SearchBar.svelte";
  import NavigationPanel from "./components/NavigationPanel.svelte";
  import Breadcrumb from "./components/Breadcrumb.svelte";
  import { filterStore } from "./stores/filter.svelte";
  import { filterGraph } from "./lib/filter-logic";
  import { navigationStore } from "./stores/navigation.svelte";
  import { expansionStore } from "./stores/expansion.svelte";
  import { focusStore } from "./stores/focus.svelte";
  import { mermaidStore } from "./stores/mermaid.svelte";
  import { viewStore, type ViewMode } from "./stores/view.svelte";
  import { extractSubtree } from "./lib/scope";
  import { computeVisibleElements } from "./lib/expansion";
  import MermaidView from "./components/MermaidView.svelte";
  import TreemapView from "./components/TreemapView.svelte";
  import ChordView from "./components/ChordView.svelte";
  import SunburstView from "./components/SunburstView.svelte";
  import BundleView from "./components/BundleView.svelte";
  import MatrixView from "./components/MatrixView.svelte";

  let showConformance = $state(false);
  let conformanceDesign = $state<Version | null>(null);
  let conformanceAnalysis = $state<Version | null>(null);
  let wasmVersion = $state<Version | null>(null);
  let compareVersion = $state<number | null>(null);
  let theme = $state<"dark" | "light">(
    (typeof localStorage !== "undefined" && localStorage.getItem("svt-theme") as "dark" | "light") || "dark"
  );

  function toggleTheme() {
    theme = theme === "dark" ? "light" : "dark";
    localStorage.setItem("svt-theme", theme);
  }

  // Apply theme to document (reactive — also handles initial value)
  $effect(() => {
    document.documentElement.dataset.theme = theme;
  });

  // Build label lookup from graph for breadcrumb display
  let labelMap = $derived.by(() => {
    const map = new Map<string, string>();
    if (graphStore.graph) {
      for (const node of graphStore.graph.elements.nodes) {
        map.set(node.data.id, node.data.label);
      }
    }
    return map;
  });

  // Build bidirectional lookups between node IDs and canonical paths for URL routing
  let idToPath = $derived.by(() => {
    const map = new Map<string, string>();
    if (graphStore.graph) {
      for (const node of graphStore.graph.elements.nodes) {
        map.set(node.data.id, node.data.canonical_path);
      }
    }
    return map;
  });
  let pathToId = $derived.by(() => {
    const map = new Map<string, string>();
    if (graphStore.graph) {
      for (const node of graphStore.graph.elements.nodes) {
        map.set(node.data.canonical_path, node.data.id);
      }
    }
    return map;
  });

  // Derive phantom node IDs from graph (nodes with sub_kind === "phantom")
  let phantomIds = $derived.by(() => {
    const ids = new Set<string>();
    if (graphStore.graph) {
      for (const node of graphStore.graph.elements.nodes) {
        if (node.data.sub_kind === "phantom") {
          ids.add(node.data.id);
        }
      }
    }
    return ids;
  });

  // Build a traversal index from the full graph for scope and navigation tree
  let fullTraversalIndex = $derived.by(() => {
    if (!graphStore.graph) return null;
    return buildTraversalIndex(graphStore.graph);
  });

  // When focus is active, extract the subtree; otherwise pass through the full graph
  let focusedGraph = $derived.by(() => {
    if (!graphStore.graph || !focusStore.focusNodeId || !fullTraversalIndex) {
      return graphStore.graph;
    }
    return extractSubtree(graphStore.graph, focusStore.focusNodeId, fullTraversalIndex);
  });

  // Build a traversal index for the focused graph (reuses fullTraversalIndex when unfocused)
  let focusedTraversalIndex = $derived.by(() => {
    if (!focusedGraph) return null;
    if (focusedGraph === graphStore.graph && fullTraversalIndex) return fullTraversalIndex;
    return buildTraversalIndex(focusedGraph);
  });

  // Apply depth-based visibility filtering to constrain MermaidView rendering
  let visibleGraph = $derived.by(() => {
    if (!focusedGraph || !focusedTraversalIndex) return focusedGraph;
    return computeVisibleElements(focusedGraph, expansionStore.expandedNodes, focusedTraversalIndex);
  });

  // Apply filter store to the visible graph for visualisations
  let filteredVisibleGraph = $derived.by(() => {
    if (!visibleGraph) return visibleGraph;
    if (!filterStore.hasActiveFilters) return visibleGraph;
    return filterGraph(
      visibleGraph,
      filterStore.nodeKinds,
      filterStore.edgeKinds,
      filterStore.subKinds,
      filterStore.languages,
      filterStore.testVisibility,
    );
  });

  /** Select a node: expand ancestors so it's visible, then select it. */
  function selectNode(nodeId: string) {
    if (fullTraversalIndex) {
      expansionStore.expandAncestors(nodeId, fullTraversalIndex);
    }
    selectionStore.selectSingle(nodeId);
    selectionStore.panelOpen = true;
  }

  // Hash routing: suppress writes during reads to avoid loops
  let suppressHashWrite = false;

  onMount(() => {
    // Listen for back/forward navigation
    function onHashChange() {
      const state = parseHash(window.location.hash);
      suppressHashWrite = true;
      if (state.version && state.version !== graphStore.selectedVersion) {
        selectVersion(state.version);
      }
      if (state.node) {
        const nodeId = pathToId.get(state.node) ?? state.node;
        selectionStore.selectedNodeId = nodeId;
        selectionStore.panelOpen = true;
      } else {
        selectionStore.clear();
      }
      if (state.diff && state.diff !== graphStore.diffVersion) {
        compareVersion = state.diff;
      }
      if (state.focusPath) {
        const focusId = pathToId.get(state.focusPath) ?? state.focusPath;
        focusStore.focus(focusId);
      } else {
        focusStore.clear();
      }
      if (state.mermaid) {
        mermaidStore.diagramType = state.mermaid as "flowchart" | "dataflow" | "sequence" | "c4";
      }
      if (state.view) {
        viewStore.setMode(state.view as ViewMode);
      }
      suppressHashWrite = false;
    }
    window.addEventListener("hashchange", onHashChange);

    // Initialize async: load WASM, snapshots, and apply initial hash state
    (async () => {
      try {
        graphStore.loading = true;
        const [, snapshots] = await Promise.all([
          initWasm(),
          api.getSnapshots(),
        ]);
        graphStore.snapshots = snapshots;

        const initial = parseHash(window.location.hash);

        const initialVersion = initial.version && snapshots.some((s) => s.version === initial.version)
          ? initial.version
          : snapshots.length > 0 ? snapshots[0].version : null;

        if (initial.mermaid) {
          mermaidStore.diagramType = initial.mermaid as "flowchart" | "dataflow" | "sequence" | "c4";
        }
        if (initial.view) {
          viewStore.setMode(initial.view as ViewMode);
        }

        if (initialVersion) {
          suppressHashWrite = true;
          await selectVersion(initialVersion);
          // Resolve canonical paths to node IDs now that the graph is loaded
          if (initial.focusPath) {
            const focusId = pathToId.get(initial.focusPath) ?? initial.focusPath;
            focusStore.focus(focusId);
          }
          if (initial.node) {
            const nodeId = pathToId.get(initial.node) ?? initial.node;
            const index = fullTraversalIndex;
            if (index) {
              expansionStore.expandAncestors(nodeId, index);
            }
            selectionStore.selectedNodeId = nodeId;
            selectionStore.panelOpen = true;
          }
          suppressHashWrite = false;
        }
      } catch (e) {
        graphStore.error = e instanceof Error ? e.message : "Failed to load";
      } finally {
        graphStore.loading = false;
      }
    })();

    return () => window.removeEventListener("hashchange", onHashChange);
  });

  async function selectVersion(version: Version) {
    try {
      graphStore.loading = true;
      graphStore.error = null;
      graphStore.selectedVersion = version;
      graphStore.conformanceReport = null;
      showConformance = false;
      compareVersion = null;
      graphStore.clearDiff();
      selectionStore.clear();
      wasmVersion = null;

      const wasmStore = getWasmStore();
      if (wasmStore) {
        // Fetch graph, nodes, and edges in parallel for WASM loading
        const [graph, nodes, edges] = await Promise.all([
          api.getGraph(version),
          api.getNodes(version),
          api.getEdges(version),
        ]);
        graphStore.graph = graph;
        filterStore.populateFromGraph(graph.elements.nodes);
        wasmVersion = wasmStore.loadSnapshot(nodes, edges);
      } else {
        const graph = await api.getGraph(version);
        graphStore.graph = graph;
        filterStore.populateFromGraph(graph.elements.nodes);
      }

      // Set default expansion: depth 2 shows system + services + components
      if (graphStore.graph) {
        const index = buildTraversalIndex(graphStore.graph);
        expansionStore.expandToDepth(2, index);
      }
    } catch (e) {
      graphStore.error = e instanceof Error ? e.message : "Failed to load graph";
    } finally {
      graphStore.loading = false;
    }
  }

  // Persist navigation panel state
  $effect(() => {
    localStorage.setItem("svt-nav-tab", navigationStore.activeTab);
    localStorage.setItem("svt-nav-collapsed", String(navigationStore.collapsed));
  });

  // Sync state to URL hash
  let previousHash = "";
  $effect(() => {
    if (suppressHashWrite) return;
    const focusId = focusStore.focusNodeId;
    const selectedId = selectionStore.selectedNodeId;
    const currentView = viewStore.mode;
    const hash = buildHash({
      version: graphStore.selectedVersion ?? undefined,
      node: selectedId ? (idToPath.get(selectedId) ?? selectedId) : undefined,
      diff: graphStore.diffVersion ?? undefined,
      focusPath: focusId ? (idToPath.get(focusId) ?? focusId) : undefined,
      mermaid: currentView === "mermaid" ? mermaidStore.diagramType : undefined,
      view: currentView ?? undefined,
    });
    if (hash !== window.location.hash) {
      // Push a new history entry when view or focus changes (for browser back/forward)
      const oldState = parseHash(previousHash);
      const newState = parseHash(hash);
      const viewChanged = oldState.view !== newState.view;
      const focusChanged = oldState.focusPath !== newState.focusPath;
      if (previousHash && (viewChanged || focusChanged)) {
        history.pushState(null, "", hash || window.location.pathname);
      } else {
        history.replaceState(null, "", hash || window.location.pathname);
      }
      previousHash = hash;
    }
  });

  // React to focus changes: expand to relative depth 2 from the focused node
  $effect(() => {
    const focusId = focusStore.focusNodeId;
    if (focusId && fullTraversalIndex) {
      expansionStore.expandToDepthFrom(2, fullTraversalIndex, focusId);
    } else if (!focusId && fullTraversalIndex) {
      expansionStore.expandToDepth(2, fullTraversalIndex);
    }
  });

  // React to node selection changes
  $effect(() => {
    const nodeId = selectionStore.selectedNodeId;
    const version = graphStore.selectedVersion;
    if (nodeId && version) {
      loadNodeDetails(version, nodeId);
    }
  });

  // React to diff comparison changes
  $effect(() => {
    if (compareVersion && graphStore.selectedVersion) {
      loadDiff(compareVersion);
    } else if (!compareVersion) {
      graphStore.clearDiff();
    }
  });

  async function loadNodeDetails(version: Version, nodeId: string) {
    selectionStore.loading = true;
    try {
      const wasmStore = getWasmStore();
      if (wasmStore && wasmVersion !== null) {
        // WASM path — zero API round-trips
        selectionStore.selectedNode = wasmStore.getNode(wasmVersion, nodeId);
        selectionStore.children = wasmStore.getChildren(wasmVersion, nodeId);
        selectionStore.ancestors = wasmStore.getAncestors(wasmVersion, nodeId);
        selectionStore.dependencies = wasmStore.getEdges(wasmVersion, nodeId, "outgoing", "depends");
        selectionStore.dependents = wasmStore.getEdges(wasmVersion, nodeId, "incoming", "depends");
      } else {
        // API fallback
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
      }
    } catch {
      // Node may not have all data — partial load is OK
    } finally {
      selectionStore.loading = false;
    }
  }

  async function handleSearch(query: string) {
    if (!graphStore.selectedVersion) return;
    try {
      const wasmStore = getWasmStore();
      let results;
      if (wasmStore && wasmVersion !== null) {
        results = wasmStore.search(wasmVersion, query);
      } else {
        results = await api.searchNodes(query, graphStore.selectedVersion);
      }
      if (results.length > 0) {
        selectNode(results[0].id);
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

  async function loadDiff(diffVersion: number) {
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

  function handleKeydown(e: KeyboardEvent) {
    // Escape: close panels, step back focus
    if (e.key === "Escape") {
      if (selectionStore.panelOpen) {
        selectionStore.clear();
        e.preventDefault();
      } else if (focusStore.active) {
        focusStore.back();
        e.preventDefault();
      } else if (showConformance) {
        clearConformance();
        e.preventDefault();
      }
      return;
    }

    // Backspace: quick back-navigation through focus history
    if (e.key === "Backspace" && !(e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement)) {
      if (focusStore.active) {
        focusStore.back();
        e.preventDefault();
        return;
      }
    }

    // Don't handle keys when focus is in an input/select
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;

    // Arrow keys: navigate graph containment hierarchy
    if (selectionStore.selectedNodeId && fullTraversalIndex) {
      const index = fullTraversalIndex;
      let targetId: string | null = null;
      if (e.key === "ArrowUp") {
        targetId = getParent(index, selectionStore.selectedNodeId);
      } else if (e.key === "ArrowDown") {
        // If node is collapsed (has children but not expanded), expand it first
        if (
          index.childrenMap.has(selectionStore.selectedNodeId) &&
          !expansionStore.isExpanded(selectionStore.selectedNodeId)
        ) {
          expansionStore.expand(selectionStore.selectedNodeId);
          // After expansion, select first child on next tick
          const firstChild = getFirstChild(index, selectionStore.selectedNodeId);
          if (firstChild) {
            requestAnimationFrame(() => selectNode(firstChild));
          }
          e.preventDefault();
          return;
        }
        targetId = getFirstChild(index, selectionStore.selectedNodeId);
      } else if (e.key === "ArrowLeft") {
        targetId = getPrevSibling(index, selectionStore.selectedNodeId);
      } else if (e.key === "ArrowRight") {
        targetId = getNextSibling(index, selectionStore.selectedNodeId);
      }
      if (targetId) {
        selectNode(targetId);
        e.preventDefault();
        return;
      }
    }

    // g: toggle navigation panel
    if (e.key === "g") {
      navigationStore.toggle();
      e.preventDefault();
    }

    // s: focus into selected node's subtree
    if (e.key === "s" && selectionStore.selectedNodeId) {
      focusStore.focus(selectionStore.selectedNodeId);
      e.preventDefault();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="app">
  <nav class="toolbar">
    <div class="toolbar-left">
      <span class="logo">Anamnetron</span>
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
      <button
        class="filter-toggle"
        onclick={() => {
          if (navigationStore.collapsed) {
            navigationStore.setTab("filters");
          } else if (navigationStore.activeTab === "filters") {
            navigationStore.collapse();
          } else {
            navigationStore.setTab("filters");
          }
        }}
        aria-label="Toggle filters"
      >Filters{#if filterStore.hasActiveFilters}<span class="filter-indicator">*</span>{/if}</button>
      <span class="view-switcher">
        {#each [
          { mode: "treemap" as ViewMode, label: "Treemap", disabled: false },
          { mode: "bundle" as ViewMode, label: "Bundle", disabled: false },
          { mode: "matrix" as ViewMode, label: "Matrix", disabled: false },
          { mode: "chord" as ViewMode, label: "Chord", disabled: false },
          { mode: "sunburst" as ViewMode, label: "Sunburst", disabled: false },
          { mode: "mermaid" as ViewMode, label: "Mermaid", disabled: false },
        ] as item}
          <button
            class="view-btn"
            class:view-btn-active={viewStore.mode === item.mode}
            disabled={item.disabled}
            onclick={() => viewStore.setMode(item.mode)}
            aria-label="Switch to {item.label} view"
          >{item.label}</button>
        {/each}
      </span>
      <span class="depth-controls">
        <button
          class="depth-btn"
          onclick={() => {
            const index = fullTraversalIndex;
            if (index && expansionStore.currentDepth > 0) {
              const newDepth = expansionStore.currentDepth - 1;
              if (focusStore.focusNodeId) {
                expansionStore.expandToDepthFrom(newDepth, index, focusStore.focusNodeId);
              } else {
                expansionStore.expandToDepth(newDepth, index);
              }
            }
          }}
          aria-label="Decrease depth"
          disabled={expansionStore.currentDepth <= 0}
        >&minus;</button>
        <span class="depth-label">Depth {expansionStore.currentDepth}</span>
        <button
          class="depth-btn"
          onclick={() => {
            const index = fullTraversalIndex;
            if (index) {
              const newDepth = expansionStore.currentDepth + 1;
              if (focusStore.focusNodeId) {
                expansionStore.expandToDepthFrom(newDepth, index, focusStore.focusNodeId);
              } else {
                expansionStore.expandToDepth(newDepth, index);
              }
            }
          }}
          aria-label="Increase depth"
        >+</button>
        <button
          class="depth-btn"
          onclick={() => {
            const index = fullTraversalIndex;
            if (index) {
              if (focusStore.focusNodeId) {
                expansionStore.expandToDepthFrom(10, index, focusStore.focusNodeId);
              } else {
                expansionStore.expandToDepth(10, index);
              }
            }
          }}
          aria-label="Expand all"
        >All</button>
      </span>
      <SearchBar onsearch={handleSearch} />
      <SnapshotSelector
        snapshots={graphStore.snapshots}
        selectedVersion={graphStore.selectedVersion}
        onselect={selectVersion}
      />
      <button class="theme-toggle" onclick={toggleTheme} aria-label="Toggle theme">
        {theme === "dark" ? "Light" : "Dark"}
      </button>
    </div>
    <div class="toolbar-right">
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

  {#if graphStore.diffReport}
    <div class="diff-bar">
      Diff: v{graphStore.diffReport.from_version} &rarr; v{graphStore.diffReport.to_version}
      &nbsp;|&nbsp;
      <span class="diff-added-count">+{graphStore.diffReport.summary.nodes_added}</span>
      <span class="diff-removed-count">-{graphStore.diffReport.summary.nodes_removed}</span>
      <span class="diff-changed-count">~{graphStore.diffReport.summary.nodes_changed}</span>
      nodes
    </div>
  {/if}

  <Breadcrumb
    selectedNodeId={selectionStore.selectedNodeId}
    traversalIndex={fullTraversalIndex}
    {labelMap}
    focusNodeId={focusStore.focusNodeId}
    onnavigate={(nodeId) => selectNode(nodeId)}
    onfocus={(nodeId) => focusStore.focus(nodeId)}
    onclearfocus={() => focusStore.clear()}
  />

  <div class="main-content">
    <NavigationPanel
      traversalIndex={fullTraversalIndex}
      {labelMap}
      {phantomIds}
      onselectnode={(nodeId) => selectNode(nodeId)}
      onfocusnode={(nodeId) => focusStore.focus(nodeId)}
    />
    <div class="graph-area">
      {#if graphStore.loading && !graphStore.graph}
        <div class="center-message">
          <div class="spinner"></div>
          <p>Loading graph data...</p>
        </div>
      {:else if focusedGraph}
        {#if viewStore.mode === "mermaid"}
          <ErrorBoundary name="Mermaid View">
            <MermaidView
              graph={filteredVisibleGraph}
              {theme}
              totalNodeCount={focusedGraph?.elements.nodes.length ?? 0}
              currentDepth={expansionStore.currentDepth}
            />
          </ErrorBoundary>
        {:else if viewStore.mode === "bundle"}
          <ErrorBoundary name="Bundle View">
            <BundleView graph={filteredVisibleGraph} />
          </ErrorBoundary>
        {:else if viewStore.mode === "matrix"}
          <ErrorBoundary name="Matrix View">
            <MatrixView graph={filteredVisibleGraph} />
          </ErrorBoundary>
        {:else if viewStore.mode === "treemap"}
          <ErrorBoundary name="Treemap View">
            <TreemapView graph={filteredVisibleGraph} />
          </ErrorBoundary>
        {:else if viewStore.mode === "chord"}
          <ErrorBoundary name="Chord View">
            <ChordView graph={filteredVisibleGraph} />
          </ErrorBoundary>
        {:else if viewStore.mode === "sunburst"}
          <ErrorBoundary name="Sunburst View">
            <SunburstView graph={filteredVisibleGraph} />
          </ErrorBoundary>
        {/if}
      {:else}
        <div class="center-message">
          <p>No data loaded</p>
          <p class="hint">Start the server with <code>--design</code> or <code>--project</code> flags.</p>
        </div>
      {/if}
    </div>

    {#if selectionStore.panelOpen}
      <ErrorBoundary name="Node Detail">
        <NodeDetail
          node={selectionStore.selectedNode}
          children={selectionStore.children}
          ancestors={selectionStore.ancestors}
          dependencies={selectionStore.dependencies}
          dependents={selectionStore.dependents}
          loading={selectionStore.loading}
        />
      </ErrorBoundary>
    {/if}

    {#if showConformance && graphStore.conformanceReport}
      <ErrorBoundary name="Conformance Report">
        <ConformanceReport report={graphStore.conformanceReport} onclose={clearConformance} />
      </ErrorBoundary>
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

  .theme-toggle {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    font-size: 0.8rem;
    padding: 0.25rem 0.5rem;
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

  .graph-area {
    flex: 1;
    position: relative;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .center-message {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: 1.2rem;
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

  .depth-controls {
    display: flex;
    align-items: center;
    gap: 0.2rem;
  }

  .depth-btn {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    font-size: 0.8rem;
    padding: 0.2rem 0.45rem;
    min-width: 1.6rem;
    text-align: center;
  }

  .depth-label {
    font-size: 0.8rem;
    color: var(--text-muted);
    min-width: 3.2rem;
    text-align: center;
  }

  .filter-toggle {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    font-size: 0.85rem;
    padding: 0.3rem 0.5rem;
    border-radius: 4px;
    cursor: pointer;
  }

  .filter-indicator {
    color: var(--accent);
    font-weight: bold;
    margin-left: 0.15rem;
  }

  .view-switcher {
    display: flex;
    align-items: center;
    gap: 0;
  }

  .view-btn {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    font-size: 0.8rem;
    padding: 0.25rem 0.5rem;
    cursor: pointer;
    border-radius: 0;
  }

  .view-btn:first-child {
    border-radius: 4px 0 0 4px;
  }

  .view-btn:last-child {
    border-radius: 0 4px 4px 0;
  }

  .view-btn + .view-btn {
    border-left: none;
  }

  .view-btn-active {
    background: var(--accent);
    color: #fff;
    border-color: var(--accent);
  }

  .view-btn-active + .view-btn {
    border-left: 1px solid var(--border);
  }

  .view-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
