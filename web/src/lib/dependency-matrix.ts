import { schemeTableau10 } from "d3-scale-chromatic";
import type { CytoscapeGraph } from "./types";

/** Aggregated dependency data between top-level modules, ready for chord layout. */
export interface DependencyMatrix {
  /** Module names (labels for arcs). */
  names: string[];
  /** Module node IDs (for selection). */
  ids: string[];
  /** Module colours (categorical). */
  colors: string[];
  /** NxN matrix where matrix[i][j] = number of dependency edges from module i to module j. */
  matrix: number[][];
}

/**
 * Identify the top-level module ancestor for a given node.
 *
 * Top-level modules are nodes at depth 1 in the containment hierarchy
 * (i.e. their parent is a root/system node that has no parent itself,
 * or they have no parent and are not the only root).
 */
function findTopLevelModule(
  nodeId: string,
  parentMap: Map<string, string | undefined>,
): string | null {
  let current = nodeId;
  let parent = parentMap.get(current);

  // Walk up the containment hierarchy until we find a node whose parent
  // is a root (has no parent itself).
  while (parent !== undefined) {
    const grandparent = parentMap.get(parent);
    if (grandparent === undefined) {
      // parent is a root node, so current is a top-level module
      return current;
    }
    current = parent;
    parent = grandparent;
  }

  // current has no parent — it is a root node, not a top-level module
  return null;
}

/**
 * Build a dependency matrix from top-level modules.
 *
 * "Top-level modules" are nodes whose parent is a root/system node.
 * Edges are aggregated: if module A has children that depend on children
 * of module B, that counts as a dependency from A to B.
 */
export function buildDependencyMatrix(graph: CytoscapeGraph): DependencyMatrix {
  // 1. Build parent lookup
  const parentMap = new Map<string, string | undefined>();
  for (const node of graph.elements.nodes) {
    parentMap.set(node.data.id, node.data.parent);
  }

  // 2. Identify top-level modules (depth-1 nodes)
  const topLevelSet = new Set<string>();
  for (const node of graph.elements.nodes) {
    const tlm = findTopLevelModule(node.data.id, parentMap);
    if (tlm !== null) {
      topLevelSet.add(tlm);
    }
  }

  // Sort for deterministic ordering
  const topLevelIds = [...topLevelSet].sort();
  const indexMap = new Map<string, number>();
  for (let i = 0; i < topLevelIds.length; i++) {
    indexMap.set(topLevelIds[i], i);
  }

  // 3. Build nodeId -> top-level module index lookup for all nodes
  const nodeToModule = new Map<string, number>();
  for (const node of graph.elements.nodes) {
    const tlm = findTopLevelModule(node.data.id, parentMap);
    if (tlm !== null) {
      const idx = indexMap.get(tlm);
      if (idx !== undefined) {
        nodeToModule.set(node.data.id, idx);
      }
    }
  }

  // 4. Initialize NxN matrix
  const n = topLevelIds.length;
  const matrix: number[][] = Array.from({ length: n }, () =>
    Array.from({ length: n }, () => 0),
  );

  // 5. Aggregate non-contains edges
  for (const edge of graph.elements.edges) {
    if (edge.data.kind === "contains") continue;

    const srcIdx = nodeToModule.get(edge.data.source);
    const tgtIdx = nodeToModule.get(edge.data.target);

    if (srcIdx !== undefined && tgtIdx !== undefined && srcIdx !== tgtIdx) {
      matrix[srcIdx][tgtIdx]++;
    }
  }

  // 6. Build labels from graph nodes
  const labelMap = new Map<string, string>();
  for (const node of graph.elements.nodes) {
    labelMap.set(node.data.id, node.data.label);
  }

  const names = topLevelIds.map((id) => labelMap.get(id) ?? id);
  const colors = topLevelIds.map((_, i) => schemeTableau10[i % schemeTableau10.length]);

  return { names, ids: topLevelIds, colors, matrix };
}
