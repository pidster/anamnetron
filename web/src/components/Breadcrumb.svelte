<script lang="ts">
  import type { TraversalIndex } from "../lib/traversal";
  import { getAncestorChain } from "../lib/expansion";

  interface Props {
    selectedNodeId: string | null;
    traversalIndex: TraversalIndex | null;
    labelMap: Map<string, string>;
    onnavigate?: (nodeId: string) => void;
  }

  let { selectedNodeId, traversalIndex, labelMap, onnavigate }: Props = $props();

  let crumbs = $derived.by(() => {
    if (!selectedNodeId || !traversalIndex) return [];
    const ancestors = getAncestorChain(traversalIndex, selectedNodeId);
    return [...ancestors, selectedNodeId].map((id) => ({
      id,
      label: labelMap.get(id) ?? id,
    }));
  });
</script>

{#if crumbs.length > 0}
  <nav class="breadcrumb" aria-label="Node path">
    {#each crumbs as crumb, i}
      {#if i > 0}<span class="separator">&gt;</span>{/if}
      {#if i < crumbs.length - 1}
        <button class="crumb" onclick={() => onnavigate?.(crumb.id)}>{crumb.label}</button>
      {:else}
        <span class="crumb current">{crumb.label}</span>
      {/if}
    {/each}
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
</style>
