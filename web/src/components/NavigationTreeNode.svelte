<script lang="ts">
  import type { TraversalIndex } from "../lib/traversal";
  import { selectionStore } from "../stores/selection.svelte";
  import NavigationTreeNode from "./NavigationTreeNode.svelte";

  interface Props {
    nodeId: string;
    traversalIndex: TraversalIndex;
    labelMap: Map<string, string>;
    depth: number;
    expandedTreeNodes: Set<string>;
    ontoggleexpand: (nodeId: string) => void;
    onselectnode?: (nodeId: string) => void;
    onscopenode?: (nodeId: string) => void;
  }

  let {
    nodeId,
    traversalIndex,
    labelMap,
    depth,
    expandedTreeNodes,
    ontoggleexpand,
    onselectnode,
    onscopenode,
  }: Props = $props();

  let children = $derived(traversalIndex.childrenMap.get(nodeId) ?? []);
  let hasChildren = $derived(children.length > 0);
  let isExpanded = $derived(expandedTreeNodes.has(nodeId));
  let isSelected = $derived(selectionStore.selectedNodeId === nodeId);
  let label = $derived(labelMap.get(nodeId) ?? nodeId);

  function handleClick(e: MouseEvent) {
    e.stopPropagation();
    onselectnode?.(nodeId);
    onscopenode?.(nodeId);
  }

  function handleChevronClick(e: MouseEvent) {
    e.stopPropagation();
    ontoggleexpand(nodeId);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.stopPropagation();
      onselectnode?.(nodeId);
    }
  }
</script>

<div class="tree-node">
  <div
    class="tree-node-row"
    class:selected={isSelected}
    style="padding-left: {depth * 16 + 4}px"
    onclick={handleClick}
    onkeydown={handleKeydown}
    role="treeitem"
    aria-expanded={hasChildren ? isExpanded : undefined}
    aria-selected={isSelected}
    tabindex="0"
  >
    {#if hasChildren}
      <button class="chevron" onclick={handleChevronClick} aria-label={isExpanded ? "Collapse" : "Expand"}>
        {isExpanded ? "\u25BE" : "\u25B8"}
      </button>
    {:else}
      <span class="chevron-spacer"></span>
    {/if}
    <span class="node-label" title={nodeId}>{label}</span>
  </div>

  {#if hasChildren && isExpanded}
    <div class="tree-children" role="group">
      {#each children as childId (childId)}
        <NavigationTreeNode
          nodeId={childId}
          {traversalIndex}
          {labelMap}
          depth={depth + 1}
          {expandedTreeNodes}
          {ontoggleexpand}
          {onselectnode}
          {onscopenode}
        />
      {/each}
    </div>
  {/if}
</div>

<style>
  .tree-node-row {
    display: flex;
    align-items: center;
    gap: 2px;
    padding-top: 2px;
    padding-bottom: 2px;
    padding-right: 8px;
    cursor: pointer;
    font-size: 0.82rem;
    color: var(--text);
    border-radius: 3px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tree-node-row:hover {
    background: var(--bg);
  }

  .tree-node-row.selected {
    background: var(--accent);
    color: #fff;
  }

  .chevron {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 0.7rem;
    width: 16px;
    height: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    flex-shrink: 0;
    padding: 0;
    border-radius: 2px;
  }

  .chevron:hover {
    background: var(--border);
    color: var(--text);
  }

  .selected .chevron {
    color: rgba(255, 255, 255, 0.8);
  }

  .selected .chevron:hover {
    background: rgba(255, 255, 255, 0.2);
    color: #fff;
  }

  .chevron-spacer {
    width: 16px;
    flex-shrink: 0;
  }

  .node-label {
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
