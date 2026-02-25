<script lang="ts">
  import type { TraversalIndex } from "../lib/traversal";
  import { getAncestorChain } from "../lib/expansion";

  interface Props {
    selectedNodeId: string | null;
    traversalIndex: TraversalIndex | null;
    labelMap: Map<string, string>;
    focusNodeId?: string | null;
    onnavigate?: (nodeId: string) => void;
    onfocus?: (nodeId: string) => void;
    onclearfocus?: () => void;
  }

  let {
    selectedNodeId,
    traversalIndex,
    labelMap,
    focusNodeId = null,
    onnavigate,
    onfocus,
    onclearfocus,
  }: Props = $props();

  // Focus breadcrumbs: full ancestor chain from graph hierarchy to focused node
  let focusCrumbs = $derived.by(() => {
    if (!focusNodeId || !traversalIndex) return [];
    const ancestors = getAncestorChain(traversalIndex, focusNodeId);
    return [...ancestors, focusNodeId].map((id, i, arr) => ({
      id,
      label: labelMap.get(id) ?? id,
      isCurrent: i === arr.length - 1,
    }));
  });

  // Selection breadcrumbs: ancestor chain of selected node (shown when no focus active)
  let selectionCrumbs = $derived.by(() => {
    if (focusNodeId || !selectedNodeId || !traversalIndex) return [];
    const ancestors = getAncestorChain(traversalIndex, selectedNodeId);
    return [...ancestors, selectedNodeId].map((id, i, arr) => ({
      id,
      label: labelMap.get(id) ?? id,
      isCurrent: i === arr.length - 1,
    }));
  });

  let showFocus = $derived(focusCrumbs.length > 0);
  let showSelection = $derived(selectionCrumbs.length > 0);
</script>

{#if showFocus || showSelection}
  <nav class="breadcrumb" aria-label="Node path">
    {#if showFocus}
      <button class="crumb root-crumb" onclick={() => onclearfocus?.()}>Root</button>
      {#each focusCrumbs as crumb}
        <span class="separator">&gt;</span>
        {#if crumb.isCurrent}
          <span class="crumb current">{crumb.label}</span>
        {:else}
          <button class="crumb" onclick={() => onfocus?.(crumb.id)}>{crumb.label}</button>
        {/if}
      {/each}
    {:else}
      {#each selectionCrumbs as crumb, i}
        {#if i > 0}<span class="separator">&gt;</span>{/if}
        {#if crumb.isCurrent}
          <span class="crumb current">{crumb.label}</span>
        {:else}
          <button class="crumb" onclick={() => onnavigate?.(crumb.id)}>{crumb.label}</button>
        {/if}
      {/each}
    {/if}
  </nav>
{/if}

<style>
  .breadcrumb {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.25rem 1rem;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    font-size: 0.8rem;
    color: var(--text-muted);
    overflow-x: auto;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .separator {
    color: var(--text-muted);
    opacity: 0.5;
  }

  .crumb {
    background: none;
    border: none;
    color: var(--accent);
    cursor: pointer;
    padding: 0.1rem 0.2rem;
    border-radius: 2px;
    font-size: 0.8rem;
  }

  .crumb:hover {
    background: var(--border);
  }

  .crumb.current {
    color: var(--text);
    cursor: default;
    font-weight: bold;
  }

  .crumb.current:hover {
    background: none;
  }

  .root-crumb {
    color: var(--text-muted);
    font-weight: 500;
  }

  .root-crumb:hover {
    color: var(--text);
    background: var(--border);
  }
</style>
