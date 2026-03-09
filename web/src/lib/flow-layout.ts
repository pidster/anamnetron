import type { CytoscapeGraph, CyNodeData, CyEdgeData } from "./types";
import type { RootAnalysis } from "../stores/flow.svelte";

/** Maximum visible nodes before module collapsing kicks in. */
const MAX_VISIBLE_NODES = 200;

/** Edge kinds representing data flow (transforms, data_flow). */
const DATA_FLOW_EDGE_KINDS = new Set(["transforms", "data_flow"]);

/** Cytoscape node definition for the flow view. */
export interface FlowNode {
  data: {
    id: string;
    label: string;
    kind: string;
    sub_kind: string;
    parent?: string;
    rootCategory?: string;
    collapsed?: boolean;
    childCount?: number;
    canonical_path?: string;
    language?: string;
  };
  classes?: string;
}

/** Cytoscape edge definition for the flow view. */
export interface FlowEdge {
  data: {
    id: string;
    source: string;
    target: string;
    kind: string;
    count?: number;
  };
}

/** Build a set of all root node IDs with their category. */
function buildRootSet(roots: RootAnalysis | null): Map<string, string> {
  const map = new Map<string, string>();
  if (!roots) return map;
  for (const r of roots.call_tree_roots) map.set(r.node_id, "call_tree_root");
  for (const r of roots.dependency_sources) map.set(r.node_id, "dependency_source");
  for (const r of roots.dependency_sinks) map.set(r.node_id, "dependency_sink");
  for (const r of roots.containment_roots) map.set(r.node_id, "containment_root");
  for (const r of roots.leaf_sinks) map.set(r.node_id, "leaf_sink");
  return map;
}

/** Build parent-children map from graph containment edges. */
function buildContainmentTree(graph: CytoscapeGraph): {
  parentMap: Map<string, string>;
  childrenMap: Map<string, string[]>;
} {
  const parentMap = new Map<string, string>();
  const childrenMap = new Map<string, string[]>();
  for (const n of graph.elements.nodes) {
    if (n.data.parent) {
      parentMap.set(n.data.id, n.data.parent);
      const siblings = childrenMap.get(n.data.parent) ?? [];
      siblings.push(n.data.id);
      childrenMap.set(n.data.parent, siblings);
    }
  }
  return { parentMap, childrenMap };
}

/** Count all descendants of a node (iterative to avoid stack overflow on deep trees). */
function countDescendants(nodeId: string, childrenMap: Map<string, string[]>): number {
  let count = 0;
  const stack = [nodeId];
  while (stack.length > 0) {
    const current = stack.pop()!;
    const children = childrenMap.get(current);
    if (!children) continue;
    count += children.length;
    for (const child of children) {
      stack.push(child);
    }
  }
  return count;
}

/** Collect all descendant IDs into a set (iterative to avoid stack overflow on deep trees). */
function collectDescendants(nodeId: string, childrenMap: Map<string, string[]>, out: Set<string>) {
  const stack = [nodeId];
  while (stack.length > 0) {
    const current = stack.pop()!;
    const children = childrenMap.get(current);
    if (!children) continue;
    for (const child of children) {
      if (!out.has(child)) {
        out.add(child);
        stack.push(child);
      }
    }
  }
}

/**
 * Convert a CytoscapeGraph into Cytoscape.js elements for the flow view.
 * Applies module collapsing when the graph exceeds MAX_VISIBLE_NODES.
 */
export function buildFlowElements(
  graph: CytoscapeGraph,
  roots: RootAnalysis | null,
  expandedNodes: Set<string>,
): { nodes: FlowNode[]; edges: FlowEdge[] } {
  const rootSet = buildRootSet(roots);
  const { parentMap, childrenMap } = buildContainmentTree(graph);
  const nodeMap = new Map<string, CyNodeData>();
  for (const n of graph.elements.nodes) {
    nodeMap.set(n.data.id, n.data);
  }

  // Determine which nodes to collapse for progressive disclosure.
  // Strategy: for very large graphs, collapse all containers;
  // for medium graphs, collapse component/unit containers.
  const collapsedNodes = new Set<string>();
  const hiddenNodes = new Set<string>();
  const totalNodes = graph.elements.nodes.length;

  if (totalNodes > MAX_VISIBLE_NODES) {
    if (totalNodes > 1000) {
      // Very large graph: collapse every container that isn't explicitly expanded.
      // Skip nodes already hidden as descendants of an earlier collapse.
      for (const [nodeId, children] of childrenMap) {
        if (children.length > 0 && !expandedNodes.has(nodeId) && !hiddenNodes.has(nodeId)) {
          collapsedNodes.add(nodeId);
          collectDescendants(nodeId, childrenMap, hiddenNodes);
        }
      }
    } else {
      // Medium graph: collapse component/unit containers
      for (const [nodeId, children] of childrenMap) {
        if (hiddenNodes.has(nodeId)) continue;
        const data = nodeMap.get(nodeId);
        if (!data) continue;
        if (data.kind === "component" || data.kind === "unit") {
          if (children.length > 0 && !expandedNodes.has(nodeId)) {
            collapsedNodes.add(nodeId);
            collectDescendants(nodeId, childrenMap, hiddenNodes);
          }
        }
      }
    }
  }

  // If still too many, collapse any visible node with children
  if (totalNodes - hiddenNodes.size > MAX_VISIBLE_NODES) {
    for (const [nodeId, children] of childrenMap) {
      if (collapsedNodes.has(nodeId) || hiddenNodes.has(nodeId)) continue;
      if (children.length > 0 && !expandedNodes.has(nodeId)) {
        collapsedNodes.add(nodeId);
        collectDescendants(nodeId, childrenMap, hiddenNodes);
      }
    }
  }

  // For explicitly expanded nodes, un-hide the node itself, its ancestors,
  // and its direct children so drill-down always works.
  for (const nodeId of expandedNodes) {
    // Un-hide the expanded node itself
    hiddenNodes.delete(nodeId);
    collapsedNodes.delete(nodeId);

    // Un-hide ancestors up to the root so the expanded node is reachable
    let ancestor = parentMap.get(nodeId);
    while (ancestor) {
      hiddenNodes.delete(ancestor);
      ancestor = parentMap.get(ancestor);
    }

    // Un-hide direct children
    const children = childrenMap.get(nodeId);
    if (!children) continue;
    for (const child of children) {
      hiddenNodes.delete(child);
    }
  }

  // Build node elements
  const nodes: FlowNode[] = [];
  for (const n of graph.elements.nodes) {
    if (hiddenNodes.has(n.data.id)) continue;

    const rootCategory = rootSet.get(n.data.id);
    const isCollapsed = collapsedNodes.has(n.data.id);
    const classes: string[] = [];
    if (rootCategory) classes.push("root", rootCategory.replace(/_/g, "-"));
    if (isCollapsed) classes.push("collapsed");
    if (n.data.kind) classes.push(`kind-${n.data.kind}`);

    const hasVisibleParent = n.data.parent && nodeMap.has(n.data.parent) && !hiddenNodes.has(n.data.parent);

    nodes.push({
      data: {
        id: n.data.id,
        label: n.data.label,
        kind: n.data.kind,
        sub_kind: n.data.sub_kind,
        parent: hasVisibleParent ? n.data.parent : undefined,
        rootCategory,
        collapsed: isCollapsed || undefined,
        childCount: isCollapsed ? countDescendants(n.data.id, childrenMap) : undefined,
        canonical_path: n.data.canonical_path,
        language: n.data.language,
      },
      classes: classes.join(" ") || undefined,
    });
  }

  // Build edge elements — skip edges involving hidden nodes, aggregate for collapsed
  const edgeAggregation = new Map<string, { count: number; kind: string }>();
  const edges: FlowEdge[] = [];

  for (const e of graph.elements.edges) {
    if (e.data.kind === "contains") continue;

    let source = e.data.source;
    let target = e.data.target;

    // Remap edges to collapsed parents
    while (hiddenNodes.has(source)) {
      const p = parentMap.get(source);
      if (!p) break;
      source = p;
    }
    while (hiddenNodes.has(target)) {
      const p = parentMap.get(target);
      if (!p) break;
      target = p;
    }

    // Skip self-loops from collapsing — except for data flow edges,
    // which represent internal transformations within collapsed modules.
    if (source === target) {
      if (!DATA_FLOW_EDGE_KINDS.has(e.data.kind)) continue;
    }
    // Skip if either endpoint is still hidden
    if (hiddenNodes.has(source) || hiddenNodes.has(target)) continue;

    const aggKey = `${source}->${target}:${e.data.kind}`;
    const existing = edgeAggregation.get(aggKey);
    if (existing) {
      existing.count++;
    } else {
      edgeAggregation.set(aggKey, { count: 1, kind: e.data.kind });
      edges.push({
        data: {
          id: aggKey,
          source,
          target,
          kind: e.data.kind,
          count: 1,
        },
      });
    }
  }

  // Update counts for aggregated edges
  for (const edge of edges) {
    const agg = edgeAggregation.get(edge.data.id);
    if (agg) edge.data.count = agg.count;
  }

  return { nodes, edges };
}

/**
 * Compute client-side root categories from graph topology.
 * Used as fallback when the roots API is unavailable.
 */
export function computeClientRoots(graph: CytoscapeGraph): RootAnalysis {
  const incomingCalls = new Map<string, number>();
  const outgoingCalls = new Map<string, number>();
  const nodeMap = new Map<string, CyNodeData>();

  for (const n of graph.elements.nodes) {
    nodeMap.set(n.data.id, n.data);
    incomingCalls.set(n.data.id, 0);
    outgoingCalls.set(n.data.id, 0);
  }

  for (const e of graph.elements.edges) {
    if (e.data.kind === "calls") {
      outgoingCalls.set(e.data.source, (outgoingCalls.get(e.data.source) ?? 0) + 1);
      incomingCalls.set(e.data.target, (incomingCalls.get(e.data.target) ?? 0) + 1);
    }
  }

  const callTreeRoots: RootAnalysis["call_tree_roots"] = [];
  const containmentRoots: RootAnalysis["containment_roots"] = [];

  for (const n of graph.elements.nodes) {
    const inc = incomingCalls.get(n.data.id) ?? 0;
    const out = outgoingCalls.get(n.data.id) ?? 0;
    // Call-tree roots: outgoing calls but no incoming calls
    if (out > 0 && inc === 0) {
      callTreeRoots.push({
        node_id: n.data.id,
        canonical_path: n.data.canonical_path,
        name: n.data.label,
      });
    }
    // Containment roots: system/service nodes without a parent
    if ((n.data.kind === "system" || n.data.kind === "service") && !n.data.parent) {
      containmentRoots.push({
        node_id: n.data.id,
        canonical_path: n.data.canonical_path,
        name: n.data.label,
      });
    }
  }

  return {
    call_tree_roots: callTreeRoots,
    dependency_sources: [],
    dependency_sinks: [],
    containment_roots: containmentRoots,
    leaf_sinks: [],
  };
}
