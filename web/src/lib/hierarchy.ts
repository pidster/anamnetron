import { hierarchy, type HierarchyNode } from "d3-hierarchy";
import type { CytoscapeGraph } from "./types";

/** A node in the d3-compatible tree structure. */
export interface TreeNode {
  id: string;
  label: string;
  kind: string;
  sub_kind: string;
  canonical_path: string;
  language?: string;
  metadata?: Record<string, unknown>;
  children: TreeNode[];
}

/** Check if a tree node is tagged as test code. */
export function isTestNode(node: TreeNode): boolean {
  const tags = node.metadata?.tags as string[] | undefined;
  return tags?.includes("test") ?? false;
}

/**
 * Build a d3-compatible hierarchy from a CytoscapeGraph.
 *
 * Uses the `parent` field on CyNodeData for containment relationships.
 * Creates a synthetic root if multiple top-level nodes exist.
 * Returns an unsummed hierarchy -- callers should call `.sum()` or
 * `.copy().sum()` with the metric appropriate for their view.
 */
export function buildHierarchy(graph: CytoscapeGraph): HierarchyNode<TreeNode> {
  // Build a lookup of id -> TreeNode and parent -> children list
  const nodeMap = new Map<string, TreeNode>();
  const childrenMap = new Map<string, TreeNode[]>();

  // First pass: create TreeNode entries
  for (const n of graph.elements.nodes) {
    const d = n.data as unknown as Record<string, unknown>;
    // Merge _childCount from collapsed nodes into metadata so getMetric can access it
    let metadata = n.data.metadata;
    if (typeof d._childCount === "number") {
      metadata = { ...metadata, _childCount: d._childCount };
    }
    const treeNode: TreeNode = {
      id: n.data.id,
      label: (d._displayLabel as string) ?? n.data.label,
      kind: n.data.kind,
      sub_kind: n.data.sub_kind,
      canonical_path: n.data.canonical_path,
      language: n.data.language,
      metadata,
      children: [],
    };
    nodeMap.set(n.data.id, treeNode);
  }

  // Second pass: attach children to parents
  const roots: TreeNode[] = [];
  for (const n of graph.elements.nodes) {
    const treeNode = nodeMap.get(n.data.id)!;
    const parentId = n.data.parent;

    if (parentId && nodeMap.has(parentId)) {
      const parent = nodeMap.get(parentId)!;
      parent.children.push(treeNode);

      if (!childrenMap.has(parentId)) {
        childrenMap.set(parentId, []);
      }
      childrenMap.get(parentId)!.push(treeNode);
    } else {
      roots.push(treeNode);
    }
  }

  // Build the root: use single root if only one, otherwise create synthetic root
  let root: TreeNode;
  if (roots.length === 1) {
    root = roots[0];
  } else {
    root = {
      id: "__root__",
      label: "Root",
      kind: "system",
      sub_kind: "",
      canonical_path: "/",
      children: roots,
    };
  }

  return hierarchy(root);
}

/**
 * Sum a hierarchy using the given metric key.
 *
 * Use `"count"` to give every leaf a value of 1 (shows module count).
 * Any other string is looked up in node metadata via `getMetric`.
 * Leaves with a zero value are given a fallback of 1 to remain visible.
 */
export function sumByMetric(
  root: HierarchyNode<TreeNode>,
  metric: string,
): HierarchyNode<TreeNode> {
  return root.sum((d) => {
    if (d.children.length > 0) return 0;
    if (metric === "count") return 1;
    const val = getMetric(d, metric);
    if (val > 0) return val;
    // Collapsed nodes may lack the metric; use _childCount as proportional fallback
    const childCount = getMetric(d, "_childCount");
    return childCount > 0 ? childCount : 1;
  });
}

/** Get a numeric metric from node metadata, defaulting to 0. */
export function getMetric(node: TreeNode, key: string): number {
  if (!node.metadata) return 0;
  const val = node.metadata[key];
  return typeof val === "number" ? val : 0;
}
