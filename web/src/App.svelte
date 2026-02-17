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
  let graphView = $state<GraphView>();
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
