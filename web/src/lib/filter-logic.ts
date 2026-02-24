import type { CyNodeData, CytoscapeGraph } from "./types";

/** Toggle an item's membership in a set, returning a new set. */
export function toggleInSet<T>(set: Set<T>, item: T): Set<T> {
  const next = new Set(set);
  if (next.has(item)) {
    next.delete(item);
  } else {
    next.add(item);
  }
  return next;
}

/** Extract unique sorted sub-kinds from graph nodes. */
export function extractSubKinds(nodes: Array<{ data: CyNodeData }>): string[] {
  const kinds = new Set<string>();
  for (const node of nodes) {
    if (node.data.sub_kind) {
      kinds.add(node.data.sub_kind);
    }
  }
  return [...kinds].sort();
}

/** Extract unique sorted languages from graph nodes. */
export function extractLanguages(nodes: Array<{ data: CyNodeData }>): string[] {
  const langs = new Set<string>();
  for (const node of nodes) {
    if (node.data.language) {
      langs.add(node.data.language);
    }
  }
  return [...langs].sort();
}

/** Check whether any filters are actively reducing the view. */
export interface FilterState {
  nodeKinds: Set<string>;
  edgeKinds: Set<string>;
  subKinds: Set<string>;
  languages: Set<string>;
  allNodeKinds: number;
  allEdgeKinds: number;
  allSubKinds: number;
  allLanguages: number;
  testVisibility: "all" | "code" | "tests";
}

/** Returns true if any filter dimension has items disabled. */
export function hasActiveFilters(state: FilterState): boolean {
  return (
    state.nodeKinds.size < state.allNodeKinds ||
    state.edgeKinds.size < state.allEdgeKinds ||
    state.subKinds.size < state.allSubKinds ||
    state.languages.size < state.allLanguages ||
    state.testVisibility !== "all"
  );
}

/**
 * Filter a graph by node kinds, sub-kinds, and languages.
 *
 * Preserves ancestor nodes (walks up the `parent` chain) so that hierarchy
 * remains intact for treemap/sunburst views. Filters edges by edgeKinds and
 * ensures both endpoints survive. Always keeps `contains` edges when both
 * parent and child survive.
 */
export function filterGraph(
  graph: CytoscapeGraph,
  nodeKinds: Set<string>,
  edgeKinds: Set<string>,
  subKinds: Set<string>,
  languages: Set<string>,
  testVisibility: "all" | "code" | "tests" = "all",
): CytoscapeGraph {
  // Build a parent lookup for ancestor preservation
  const parentMap = new Map<string, string | undefined>();
  for (const node of graph.elements.nodes) {
    parentMap.set(node.data.id, node.data.parent);
  }

  // First pass: determine which nodes directly pass filters
  const directPass = new Set<string>();
  for (const node of graph.elements.nodes) {
    const d = node.data;
    const kindMatch = nodeKinds.has(d.kind);
    const subKindMatch = !d.sub_kind || subKinds.has(d.sub_kind);
    const langMatch = !d.language || languages.has(d.language);
    const tags = (d.metadata?.tags as string[] | undefined) ?? [];
    const hasTestTag = tags.includes("test");
    const testMatch =
      testVisibility === "all" ||
      (testVisibility === "code" && !hasTestTag) ||
      (testVisibility === "tests" && hasTestTag);
    if (kindMatch && subKindMatch && langMatch && testMatch) {
      directPass.add(d.id);
    }
  }

  // Second pass: preserve ancestors of passing nodes
  const surviving = new Set(directPass);
  for (const nodeId of directPass) {
    let current = parentMap.get(nodeId);
    while (current !== undefined) {
      if (surviving.has(current)) break;
      surviving.add(current);
      current = parentMap.get(current);
    }
  }

  // Filter nodes
  const filteredNodes = graph.elements.nodes.filter((n) => surviving.has(n.data.id));

  // Filter edges: both endpoints must survive, and edge kind must be allowed
  // (always keep "contains" edges when both endpoints survive)
  const filteredEdges = graph.elements.edges.filter((e) => {
    if (!surviving.has(e.data.source) || !surviving.has(e.data.target)) return false;
    if (e.data.kind === "contains") return true;
    return edgeKinds.has(e.data.kind);
  });

  return {
    elements: {
      nodes: filteredNodes,
      edges: filteredEdges,
    },
  };
}

/** Format a snake_case or lowercase string into Title Case. */
export function formatLabel(value: string): string {
  return value
    .split("_")
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}
