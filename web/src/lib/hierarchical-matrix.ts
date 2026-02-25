import type { CytoscapeGraph } from "./types";

/** A visible node in the flattened matrix ordering. */
export interface MatrixNode {
  id: string;
  label: string;
  kind: string;
  depth: number;
  hasChildren: boolean;
  expanded: boolean;
}

/** A cell in the sparse matrix. */
export interface MatrixCell {
  /** Row index. */
  row: number;
  /** Column index. */
  col: number;
  /** Number of aggregated dependency edges. */
  count: number;
  /** True if the reverse direction also has edges (cycle). */
  isCyclic: boolean;
}

/** Sort modes for the matrix. */
export type MatrixSortMode = "hierarchy" | "alphabetical" | "dependency-count";

/** Result of building the hierarchical matrix. */
export interface HierarchicalMatrix {
  /** Ordered list of visible nodes (rows/columns). */
  nodes: MatrixNode[];
  /** Sparse set of cells with non-zero counts. */
  cells: MatrixCell[];
  /** Maximum count across all cells. */
  maxCount: number;
}

/**
 * Build a hierarchical dependency structure matrix.
 *
 * The matrix shows visible nodes (based on expansion state) as rows and columns.
 * Edges from collapsed subtrees are aggregated to their nearest visible ancestor.
 */
export function buildHierarchicalMatrix(
  graph: CytoscapeGraph,
  expandedNodes: Set<string>,
  sortMode: MatrixSortMode = "hierarchy",
): HierarchicalMatrix {
  if (graph.elements.nodes.length === 0) {
    return { nodes: [], cells: [], maxCount: 0 };
  }

  // Build parent lookup and children map
  const parentMap = new Map<string, string | undefined>();
  const childrenMap = new Map<string, string[]>();
  const labelMap = new Map<string, string>();
  const kindMap = new Map<string, string>();

  for (const node of graph.elements.nodes) {
    parentMap.set(node.data.id, node.data.parent);
    labelMap.set(node.data.id, node.data.label);
    kindMap.set(node.data.id, node.data.kind);

    if (node.data.parent) {
      const siblings = childrenMap.get(node.data.parent);
      if (siblings) {
        siblings.push(node.data.id);
      } else {
        childrenMap.set(node.data.parent, [node.data.id]);
      }
    }
  }

  // Find root nodes (no parent)
  const roots: string[] = [];
  for (const node of graph.elements.nodes) {
    if (!node.data.parent || !parentMap.has(node.data.parent)) {
      roots.push(node.data.id);
    }
  }

  // Depth-first traversal to get visible nodes
  const visibleNodes: MatrixNode[] = [];
  const nodeToVisibleAncestor = new Map<string, string>();

  function traverse(nodeId: string, depth: number) {
    const children = childrenMap.get(nodeId) ?? [];
    const hasChildren = children.length > 0;
    const isExpanded = expandedNodes.has(nodeId);

    visibleNodes.push({
      id: nodeId,
      label: labelMap.get(nodeId) ?? nodeId,
      kind: kindMap.get(nodeId) ?? "component",
      depth,
      hasChildren,
      expanded: isExpanded && hasChildren,
    });

    // Map this node to itself as visible ancestor
    nodeToVisibleAncestor.set(nodeId, nodeId);

    if (isExpanded && hasChildren) {
      const sorted = sortChildren(children, sortMode, labelMap);
      for (const childId of sorted) {
        traverse(childId, depth + 1);
      }
    } else if (hasChildren) {
      // Map all descendants to this collapsed node
      mapDescendants(nodeId, nodeId);
    }
  }

  function mapDescendants(nodeId: string, visibleAncestorId: string) {
    const children = childrenMap.get(nodeId) ?? [];
    for (const childId of children) {
      nodeToVisibleAncestor.set(childId, visibleAncestorId);
      mapDescendants(childId, visibleAncestorId);
    }
  }

  const sortedRoots = sortChildren(roots, sortMode, labelMap);
  for (const rootId of sortedRoots) {
    traverse(rootId, 0);
  }

  // Build index from visible node ID to matrix index
  const indexMap = new Map<string, number>();
  for (let i = 0; i < visibleNodes.length; i++) {
    indexMap.set(visibleNodes[i].id, i);
  }

  // Aggregate edges to nearest visible ancestors
  const cellMap = new Map<string, number>(); // "row,col" -> count

  for (const edge of graph.elements.edges) {
    if (edge.data.kind === "contains") continue;

    const sourceVisible = nodeToVisibleAncestor.get(edge.data.source);
    const targetVisible = nodeToVisibleAncestor.get(edge.data.target);

    if (!sourceVisible || !targetVisible) continue;
    if (sourceVisible === targetVisible) continue; // Skip self-edges

    const row = indexMap.get(sourceVisible);
    const col = indexMap.get(targetVisible);
    if (row === undefined || col === undefined) continue;

    const key = `${row},${col}`;
    cellMap.set(key, (cellMap.get(key) ?? 0) + 1);
  }

  // Convert to sparse cell array and detect cycles
  const cells: MatrixCell[] = [];
  let maxCount = 0;

  for (const [key, count] of cellMap) {
    const [row, col] = key.split(",").map(Number);
    const reverseKey = `${col},${row}`;
    const isCyclic = cellMap.has(reverseKey);

    cells.push({ row, col, count, isCyclic });
    if (count > maxCount) maxCount = count;
  }

  // Sort by dependency count if requested (re-order the visible nodes)
  if (sortMode === "dependency-count") {
    const depCounts = new Map<number, number>();
    for (const cell of cells) {
      depCounts.set(cell.row, (depCounts.get(cell.row) ?? 0) + cell.count);
      depCounts.set(cell.col, (depCounts.get(cell.col) ?? 0) + cell.count);
    }

    // Build a permutation index
    const indices = visibleNodes.map((_, i) => i);
    indices.sort((a, b) => (depCounts.get(b) ?? 0) - (depCounts.get(a) ?? 0));

    // Apply permutation
    const reordered = indices.map((i) => visibleNodes[i]);
    const remap = new Map<number, number>();
    for (let i = 0; i < indices.length; i++) {
      remap.set(indices[i], i);
    }

    const remappedCells = cells.map((cell) => ({
      ...cell,
      row: remap.get(cell.row) ?? cell.row,
      col: remap.get(cell.col) ?? cell.col,
    }));

    return { nodes: reordered, cells: remappedCells, maxCount };
  }

  return { nodes: visibleNodes, cells, maxCount };
}

/** Sort children by the given mode. */
function sortChildren(
  children: string[],
  sortMode: MatrixSortMode,
  labelMap: Map<string, string>,
): string[] {
  if (sortMode === "alphabetical") {
    return [...children].sort((a, b) =>
      (labelMap.get(a) ?? a).localeCompare(labelMap.get(b) ?? b),
    );
  }
  // hierarchy and dependency-count use natural order for initial traversal
  return children;
}
