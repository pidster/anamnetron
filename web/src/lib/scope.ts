import type { CytoscapeGraph } from "./types";
import type { TraversalIndex } from "./traversal";

/**
 * Extract the subtree rooted at `rootId` from the full graph.
 *
 * Returns a new CytoscapeGraph containing only the root node and all its
 * descendants. Edges are included only when both endpoints are in the subtree.
 *
 * If `rootId` is not found in the graph, returns the original graph unchanged.
 */
export function extractSubtree(
  graph: CytoscapeGraph,
  rootId: string,
  index: TraversalIndex,
): CytoscapeGraph {
  // Collect all descendant IDs (including the root itself)
  const subtreeIds = new Set<string>();
  collectDescendants(rootId, index, subtreeIds);

  // If the root wasn't found in the graph, return unchanged
  if (subtreeIds.size === 0) return graph;

  // Filter nodes: include only those in the subtree
  const nodes = graph.elements.nodes.filter((n) => subtreeIds.has(n.data.id)).map((n) => {
    // The root node should have no parent in the scoped view
    if (n.data.id === rootId && n.data.parent) {
      return { data: { ...n.data, parent: undefined } };
    }
    return { data: { ...n.data } };
  });

  // Filter edges: include only those where both endpoints are in the subtree
  const edges = graph.elements.edges.filter(
    (e) => subtreeIds.has(e.data.source) && subtreeIds.has(e.data.target),
  ).map((e) => ({ data: { ...e.data } }));

  return { elements: { nodes, edges } };
}

/** Recursively collect a node and all its descendants into the set. */
function collectDescendants(
  nodeId: string,
  index: TraversalIndex,
  result: Set<string>,
): void {
  result.add(nodeId);
  const children = index.childrenMap.get(nodeId);
  if (children) {
    for (const child of children) {
      collectDescendants(child, index, result);
    }
  }
}
