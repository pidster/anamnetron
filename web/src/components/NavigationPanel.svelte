<script lang="ts">
  import { navigationStore } from "../stores/navigation.svelte";
  import FilterPanel from "./FilterPanel.svelte";
  import NavigationTree from "./NavigationTree.svelte";
  import type { TraversalIndex } from "../lib/traversal";

  interface Props {
    traversalIndex: TraversalIndex | null;
    labelMap: Map<string, string>;
    phantomIds?: Set<string>;
    onselectnode?: (nodeId: string) => void;
    onscopenode?: (nodeId: string) => void;
  }

  let { traversalIndex, labelMap, phantomIds = new Set(), onselectnode, onscopenode }: Props = $props();
</script>

<aside class="nav-panel" class:collapsed={navigationStore.collapsed}>
  {#if navigationStore.collapsed}
    <button
      class="expand-btn"
      onclick={() => navigationStore.expand()}
      aria-label="Expand navigation panel"
    >&#9654;</button>
  {:else}
    <div class="panel-header">
      <div class="tabs">
        <button
          class="tab"
          class:active={navigationStore.activeTab === "tree"}
          onclick={() => navigationStore.setTab("tree")}
        >Tree</button>
        <button
          class="tab"
          class:active={navigationStore.activeTab === "filters"}
          onclick={() => navigationStore.setTab("filters")}
        >Filters</button>
      </div>
      <button
        class="collapse-btn"
        onclick={() => navigationStore.collapse()}
        aria-label="Collapse navigation panel"
      >&#9664;</button>
    </div>

    <div class="panel-content">
      {#if navigationStore.activeTab === "tree"}
        {#if traversalIndex}
          <NavigationTree
            {traversalIndex}
            {labelMap}
            {phantomIds}
            {onselectnode}
            {onscopenode}
          />
        {:else}
          <div class="empty-state">No graph loaded</div>
        {/if}
      {:else}
        <FilterPanel />
      {/if}
    </div>
  {/if}
</aside>

<style>
  .nav-panel {
    width: var(--nav-width, 260px);
    min-width: var(--nav-width, 260px);
    background: var(--surface);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    transition: width 200ms ease, min-width 200ms ease;
    overflow: hidden;
  }

  .nav-panel.collapsed {
    width: var(--nav-collapsed-width, 32px);
    min-width: var(--nav-collapsed-width, 32px);
  }

  .panel-header {
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .tabs {
    display: flex;
    flex: 1;
  }

  .tab {
    flex: 1;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--text-muted);
    font-size: 0.82rem;
    padding: 0.5rem 0.5rem;
    cursor: pointer;
    text-align: center;
    border-radius: 0;
  }

  .tab:hover {
    color: var(--text);
    background: var(--bg);
  }

  .tab.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
    font-weight: 600;
  }

  .collapse-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 0.7rem;
    padding: 0.5rem;
    cursor: pointer;
    flex-shrink: 0;
  }

  .collapse-btn:hover {
    color: var(--text);
  }

  .expand-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 0.7rem;
    width: 100%;
    padding: 0.5rem 0;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    margin-top: 0.5rem;
  }

  .expand-btn:hover {
    color: var(--text);
    background: var(--bg);
  }

  .panel-content {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }

  .empty-state {
    padding: 1rem;
    color: var(--text-muted);
    font-size: 0.85rem;
    text-align: center;
  }
</style>
