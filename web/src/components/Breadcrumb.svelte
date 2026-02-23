<script lang="ts">
  import type { TraversalIndex } from "../lib/traversal";
  import { getAncestorChain } from "../lib/expansion";

  interface Props {
    selectedNodeId: string | null;
    traversalIndex: TraversalIndex | null;
    labelMap: Map<string, string>;
    scopeNodeId?: string | null;
    onnavigate?: (nodeId: string) => void;
    onclearscope?: () => void;
  }

  let { selectedNodeId, traversalIndex, labelMap, scopeNodeId = null, onnavigate, onclearscope }: Props = $props();

  let scopeLabel = $derived(scopeNodeId ? (labelMap.get(scopeNodeId) ?? scopeNodeId) : null);

  let crumbs = $derived.by(() => {
    if (!selectedNodeId || !traversalIndex) return [];
    const ancestors = getAncestorChain(traversalIndex, selectedNodeId);
    return [...ancestors, selectedNodeId].map((id) => ({
      id,
      label: labelMap.get(id) ?? id,
    }));
  });
</script>

{#if crumbs.length > 0 || scopeLabel}
  <nav class="breadcrumb" aria-label="Node path">
    {#if scopeLabel}
      <span class="scope-chip">
        Scope: {scopeLabel}
        <button class="scope-clear" onclick={() => onclearscope?.()} aria-label="Clear scope">&times;</button>
      </span>
      {#if crumbs.length > 0}
        <span class="separator">|</span>
      {/if}
    {/if}
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

  .scope-chip {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    background: var(--accent);
    color: #fff;
    padding: 0.1rem 0.4rem;
    border-radius: 3px;
    font-size: 0.78rem;
    font-weight: 600;
  }

  .scope-clear {
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.8);
    cursor: pointer;
    font-size: 0.9rem;
    padding: 0 0.1rem;
    line-height: 1;
    border-radius: 2px;
  }

  .scope-clear:hover {
    color: #fff;
    background: rgba(255, 255, 255, 0.2);
  }
</style>
