import type { CytoscapeGraph } from "./types";

/** Pre-computed index for O(1) graph traversal lookups. */
export interface TraversalIndex {
  /** nodeId -> parentId (absent for root nodes) */
  parentMap: Map<string, string>;
  /** nodeId -> sorted child IDs (alphabetical by label) */
  childrenMap: Map<string, string[]>;
  /** nodeId -> sorted sibling IDs including self (alphabetical by label) */
  siblingsMap: Map<string, string[]>;
}

/**
 * Build a traversal index from a Cytoscape graph.
 *
 * Children and siblings are sorted alphabetically by label for
 * deterministic navigation order.
 */
export function buildTraversalIndex(graph: CytoscapeGraph): TraversalIndex {
  const parentMap = new Map<string, string>();
  const childrenMap = new Map<string, string[]>();
  const siblingsMap = new Map<string, string[]>();

  // Build label lookup for sorting
  const labelOf = new Map<string, string>();
  for (const node of graph.elements.nodes) {
    labelOf.set(node.data.id, node.data.label ?? node.data.id);
  }

  const sortByLabel = (ids: string[]): string[] =>
    [...ids].sort((a, b) =>
      (labelOf.get(a) ?? a).localeCompare(labelOf.get(b) ?? b),
    );

  // Group children by parent
  const parentToChildren = new Map<string, string[]>();
  const rootIds: string[] = [];

  for (const node of graph.elements.nodes) {
    const { id, parent } = node.data;
    if (parent) {
      parentMap.set(id, parent);
      const siblings = parentToChildren.get(parent) ?? [];
      siblings.push(id);
      parentToChildren.set(parent, siblings);
    } else {
      rootIds.push(id);
    }
  }

  // Sort children lists and populate childrenMap
  for (const [parentId, kids] of parentToChildren) {
    childrenMap.set(parentId, sortByLabel(kids));
  }

  // Build sibling lists (root nodes are siblings of each other)
  const sortedRoots = sortByLabel(rootIds);
  for (const id of sortedRoots) {
    siblingsMap.set(id, sortedRoots);
  }
  for (const [, kids] of childrenMap) {
    for (const kid of kids) {
      siblingsMap.set(kid, kids);
    }
  }

  return { parentMap, childrenMap, siblingsMap };
}

/** Get the parent node ID, or null for root nodes. */
export function getParent(
  index: TraversalIndex,
  nodeId: string,
): string | null {
  return index.parentMap.get(nodeId) ?? null;
}

/** Get the first child (alphabetically by label), or null for leaf nodes. */
export function getFirstChild(
  index: TraversalIndex,
  nodeId: string,
): string | null {
  const children = index.childrenMap.get(nodeId);
  return children && children.length > 0 ? children[0] : null;
}

/** Get the next sibling (wraps around), or null if no siblings. */
export function getNextSibling(
  index: TraversalIndex,
  nodeId: string,
): string | null {
  const siblings = index.siblingsMap.get(nodeId);
  if (!siblings || siblings.length <= 1) return null;
  const idx = siblings.indexOf(nodeId);
  if (idx === -1) return null;
  return siblings[(idx + 1) % siblings.length];
}

/** Get the previous sibling (wraps around), or null if no siblings. */
export function getPrevSibling(
  index: TraversalIndex,
  nodeId: string,
): string | null {
  const siblings = index.siblingsMap.get(nodeId);
  if (!siblings || siblings.length <= 1) return null;
  const idx = siblings.indexOf(nodeId);
  if (idx === -1) return null;
  return siblings[(idx - 1 + siblings.length) % siblings.length];
}
