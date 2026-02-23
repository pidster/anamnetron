import type { CytoscapeGraph, CyEdgeData } from "./types";
import type { TraversalIndex } from "./traversal";

/**
 * Compute the subset of graph elements visible given the current expansion state.
 *
 * A node is visible if every ancestor in the containment hierarchy is expanded.
 * Collapsed parent nodes become leaf nodes with a `_childCount` data attribute.
 * Edges are included only when both endpoints are visible.
 */
export function computeVisibleElements(
  fullGraph: CytoscapeGraph,
  expandedNodes: Set<string>,
  index: TraversalIndex,
): CytoscapeGraph {
  const visibleIds = new Set<string>();

  for (const node of fullGraph.elements.nodes) {
    const id = node.data.id;
    const parent = index.parentMap.get(id);

    if (!parent) {
      // Root node — always visible
      visibleIds.add(id);
    } else if (isAncestorChainExpanded(id, expandedNodes, index)) {
      visibleIds.add(id);
    }
  }

  // Build visible nodes, attaching _childCount to collapsed parents
  const nodes = fullGraph.elements.nodes
    .filter((n) => visibleIds.has(n.data.id))
    .map((n) => {
      const id = n.data.id;
      const children = index.childrenMap.get(id);
      const hasChildren = children && children.length > 0;
      const isExpanded = expandedNodes.has(id);

      if (hasChildren && !isExpanded) {
        // Collapsed parent — attach descendant count, remove parent role
        return {
          data: {
            ...n.data,
            _childCount: countDescendants(index, id),
          },
        };
      }
      return { data: { ...n.data } };
    });

  // Edges: include as-is when both endpoints visible, otherwise aggregate into meta-edges
  const edges: Array<{ data: CyEdgeData }> = [];
  const metaAccum = new Map<string, { source: string; target: string; kind: string; count: number }>();

  for (const e of fullGraph.elements.edges) {
    const src = e.data.source;
    const tgt = e.data.target;

    if (visibleIds.has(src) && visibleIds.has(tgt)) {
      // Both endpoints visible — include as-is
      edges.push({ data: { ...e.data } });
    } else {
      // Resolve to nearest visible ancestor
      const visSrc = visibleIds.has(src) ? src : findVisibleAncestor(src, visibleIds, index);
      const visTgt = visibleIds.has(tgt) ? tgt : findVisibleAncestor(tgt, visibleIds, index);
      if (!visSrc || !visTgt || visSrc === visTgt) continue; // internal to same subtree

      const metaKey = `${visSrc}\0${visTgt}\0${e.data.kind}`;
      const existing = metaAccum.get(metaKey);
      if (existing) {
        existing.count++;
      } else {
        metaAccum.set(metaKey, { source: visSrc, target: visTgt, kind: e.data.kind, count: 1 });
      }
    }
  }

  // Emit accumulated meta-edges
  for (const [, meta] of metaAccum) {
    edges.push({
      data: {
        id: `meta:${meta.source}:${meta.target}:${meta.kind}`,
        source: meta.source,
        target: meta.target,
        kind: meta.kind,
        _isMeta: true,
        _count: meta.count,
      },
    });
  }

  return { elements: { nodes, edges } };
}

/** Walk up the containment tree to find the nearest visible ancestor, or null if none. */
function findVisibleAncestor(
  nodeId: string,
  visibleIds: Set<string>,
  index: TraversalIndex,
): string | null {
  let current = index.parentMap.get(nodeId);
  while (current !== undefined) {
    if (visibleIds.has(current)) return current;
    current = index.parentMap.get(current);
  }
  return null;
}

/** Check whether every ancestor of a node is expanded. */
function isAncestorChainExpanded(
  nodeId: string,
  expandedNodes: Set<string>,
  index: TraversalIndex,
): boolean {
  let current = index.parentMap.get(nodeId);
  while (current !== undefined) {
    if (!expandedNodes.has(current)) return false;
    current = index.parentMap.get(current);
  }
  return true;
}

/**
 * Compute the default set of expanded node IDs for a given depth.
 *
 * Depth 0 = nothing expanded (only roots visible).
 * Depth 1 = roots expanded (their children visible).
 * Depth 2 = roots + depth-1 children expanded, etc.
 */
export function computeDefaultExpansion(
  index: TraversalIndex,
  maxDepth: number,
): Set<string> {
  const expanded = new Set<string>();
  if (maxDepth <= 0) return expanded;

  // BFS from roots
  // Roots are nodes not in parentMap
  const allChildren = new Set<string>(index.parentMap.keys());
  const roots: string[] = [];
  for (const [parentId] of index.childrenMap) {
    if (!allChildren.has(parentId) || !index.parentMap.has(parentId)) {
      roots.push(parentId);
    }
  }
  // Also include root nodes that have no children but are roots
  // We only need to expand nodes that have children
  let frontier = roots;
  let depth = 0;

  while (depth < maxDepth && frontier.length > 0) {
    const nextFrontier: string[] = [];
    for (const id of frontier) {
      expanded.add(id);
      const children = index.childrenMap.get(id);
      if (children) {
        nextFrontier.push(...children);
      }
    }
    frontier = nextFrontier;
    depth++;
  }

  return expanded;
}

/**
 * Return the ancestor chain from root down to (but not including) the given node.
 * Result: `[root, ..., grandparent, parent]`.
 */
export function getAncestorChain(
  index: TraversalIndex,
  nodeId: string,
): string[] {
  const chain: string[] = [];
  let current = index.parentMap.get(nodeId);
  while (current !== undefined) {
    chain.unshift(current);
    current = index.parentMap.get(current);
  }
  return chain;
}

/** Count all descendants (children, grandchildren, etc.) of a node. */
export function countDescendants(
  index: TraversalIndex,
  nodeId: string,
): number {
  const children = index.childrenMap.get(nodeId);
  if (!children || children.length === 0) return 0;
  let count = children.length;
  for (const child of children) {
    count += countDescendants(index, child);
  }
  return count;
}
