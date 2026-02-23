<script lang="ts">
  import type { TraversalIndex } from "../lib/traversal";
  import NavigationTreeNode from "./NavigationTreeNode.svelte";

  interface Props {
    traversalIndex: TraversalIndex;
    labelMap: Map<string, string>;
    phantomIds?: Set<string>;
    onselectnode?: (nodeId: string) => void;
    onscopenode?: (nodeId: string) => void;
  }

  let { traversalIndex, labelMap, phantomIds = new Set(), onselectnode, onscopenode }: Props = $props();

  // Track which tree nodes are expanded (independent from graph expansion)
  let expandedTreeNodes = $state<Set<string>>(new Set());

  // Derive root nodes: nodes that have children but no parent in the index
  let rootNodes = $derived.by(() => {
    const roots: string[] = [];
    const allChildIds = new Set(traversalIndex.parentMap.keys());

    // Collect all nodes: parents + leaf nodes without parents
    const allNodeIds = new Set<string>();
    for (const [parentId] of traversalIndex.childrenMap) {
      allNodeIds.add(parentId);
    }
    for (const [childId] of traversalIndex.parentMap) {
      allNodeIds.add(childId);
    }
    // Also add nodes from siblings map that have no parent
    for (const [nodeId] of traversalIndex.siblingsMap) {
      allNodeIds.add(nodeId);
    }

    for (const nodeId of allNodeIds) {
      if (!traversalIndex.parentMap.has(nodeId)) {
        roots.push(nodeId);
      }
    }

    // Sort by label
    return roots.sort((a, b) =>
      (labelMap.get(a) ?? a).localeCompare(labelMap.get(b) ?? b)
    );
  });

  // Auto-expand root nodes on first load
  $effect(() => {
    if (rootNodes.length > 0 && expandedTreeNodes.size === 0) {
      const initial = new Set<string>();
      for (const root of rootNodes) {
        initial.add(root);
      }
      expandedTreeNodes = initial;
    }
  });

  function toggleExpand(nodeId: string) {
    const next = new Set(expandedTreeNodes);
    if (next.has(nodeId)) {
      next.delete(nodeId);
    } else {
      next.add(nodeId);
    }
    expandedTreeNodes = next;
  }
</script>

<div class="nav-tree" role="tree">
  {#each rootNodes as rootId (rootId)}
    <NavigationTreeNode
      nodeId={rootId}
      {traversalIndex}
      {labelMap}
      {phantomIds}
      depth={0}
      {expandedTreeNodes}
      ontoggleexpand={toggleExpand}
      {onselectnode}
      {onscopenode}
    />
  {/each}
</div>

<style>
  .nav-tree {
    padding: 0.25rem 0;
  }
</style>
