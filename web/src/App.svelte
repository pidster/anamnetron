<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "./lib/api";
  import type { Version, SnapshotDiff } from "./lib/types";
  import { graphStore } from "./stores/graph.svelte";
  import { selectionStore } from "./stores/selection.svelte";
  import { initWasm, getWasmStore } from "./lib/wasm";
  import { parseHash, buildHash } from "./lib/router";
  import GraphView from "./components/GraphView.svelte";
  import NodeDetail from "./components/NodeDetail.svelte";
  import ConformanceReport from "./components/ConformanceReport.svelte";
  import SnapshotSelector from "./components/SnapshotSelector.svelte";
  import SearchBar from "./components/SearchBar.svelte";

  const savedLayout = typeof localStorage !== "undefined" ? localStorage.getItem("svt-layout") : null;
  let layoutChoice = $state<"cose-bilkent" | "dagre">(
    savedLayout === "dagre" ? "dagre" : "cose-bilkent"
  );
  let graphView = $state<GraphView>();
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
    document.documentElement.dataset.theme = theme;
    localStorage.setItem("svt-theme", theme);
  }

  // Apply initial theme
  if (typeof document !== "undefined") {
    document.documentElement.dataset.theme = theme;
  }

  // Hash routing: suppress writes during reads to avoid loops
  let suppressHashWrite = false;

  onMount(async () => {
    try {
      graphStore.loading = true;
      // Initialize WASM and load snapshots in parallel
      const [, snapshots] = await Promise.all([
        initWasm(),
        api.getSnapshots(),
      ]);
      graphStore.snapshots = snapshots;

      // Apply initial state from hash
      const initial = parseHash(window.location.hash);
      if (initial.layout === "dagre" || initial.layout === "cose-bilkent") {
        layoutChoice = initial.layout;
      }

      const initialVersion = initial.version && snapshots.some((s) => s.version === initial.version)
        ? initial.version
        : snapshots.length > 0 ? snapshots[0].version : null;

      if (initialVersion) {
        suppressHashWrite = true;
        await selectVersion(initialVersion);
        if (initial.node) {
          selectionStore.selectedNodeId = initial.node;
          selectionStore.panelOpen = true;
        }
        suppressHashWrite = false;
      }
    } catch (e) {
      graphStore.error = e instanceof Error ? e.message : "Failed to load";
    } finally {
      graphStore.loading = false;
    }

    // Listen for back/forward navigation
    function onHashChange() {
      const state = parseHash(window.location.hash);
      suppressHashWrite = true;
      if (state.version && state.version !== graphStore.selectedVersion) {
        selectVersion(state.version);
      }
      if (state.node) {
        selectionStore.selectedNodeId = state.node;
        selectionStore.panelOpen = true;
      } else {
        selectionStore.clear();
      }
      if (state.layout === "dagre" || state.layout === "cose-bilkent") {
        layoutChoice = state.layout;
      }
      if (state.diff && state.diff !== graphStore.diffVersion) {
        compareVersion = state.diff;
      }
      suppressHashWrite = false;
    }
    window.addEventListener("hashchange", onHashChange);
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
        wasmVersion = wasmStore.loadSnapshot(nodes, edges);
      } else {
        graphStore.graph = await api.getGraph(version);
      }
    } catch (e) {
      graphStore.error = e instanceof Error ? e.message : "Failed to load graph";
    } finally {
      graphStore.loading = false;
    }
  }

  // Persist layout choice
  $effect(() => {
    localStorage.setItem("svt-layout", layoutChoice);
  });

  // Sync state to URL hash
  $effect(() => {
    if (suppressHashWrite) return;
    const hash = buildHash({
      version: graphStore.selectedVersion ?? undefined,
      node: selectionStore.selectedNodeId ?? undefined,
      layout: layoutChoice,
      diff: graphStore.diffVersion ?? undefined,
    });
    if (hash !== window.location.hash) {
      history.replaceState(null, "", hash || window.location.pathname);
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

    // Don't handle keys when focus is in an input/select
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;

    // f: fit all elements in viewport
    if (e.key === "f") {
      graphView?.fitAll();
      e.preventDefault();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="app">
  <nav class="toolbar">
    <div class="toolbar-left">
      <span class="logo">SVT</span>
      <button class="theme-toggle" onclick={toggleTheme} aria-label="Toggle theme">
        {theme === "dark" ? "Light" : "Dark"}
      </button>
      <SnapshotSelector
        snapshots={graphStore.snapshots}
        selectedVersion={graphStore.selectedVersion}
        onselect={selectVersion}
      />
      <SearchBar onsearch={handleSearch} />
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

  <div class="main-content">
    {#if graphStore.loading && !graphStore.graph}
      <div class="center-message">
        <div class="spinner"></div>
        <p>Loading graph data...</p>
      </div>
    {:else if graphStore.graph}
      <GraphView
        bind:this={graphView}
        graph={graphStore.graph}
        conformance={graphStore.conformanceReport}
        diff={graphStore.diffReport}
        layout={layoutChoice}
        {theme}
      />
    {:else}
      <div class="center-message">
        <p>No data loaded</p>
        <p class="hint">Start the server with <code>--design</code> or <code>--project</code> flags.</p>
      </div>
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
</style>
