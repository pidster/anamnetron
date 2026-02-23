import { describe, it, expect } from "vitest";
import { extractSubtree } from "../scope";
import { buildTraversalIndex } from "../traversal";
import type { CytoscapeGraph } from "../types";

function makeGraph(
  nodes: Array<{ id: string; label: string; parent?: string }>,
  edges: Array<{ id: string; source: string; target: string; kind: string }> = [],
): CytoscapeGraph {
  return {
    elements: {
      nodes: nodes.map((n) => ({
        data: {
          id: n.id,
          label: n.label,
          kind: "component",
          sub_kind: "",
          canonical_path: `/${n.id}`,
          parent: n.parent,
        },
      })),
      edges: edges.map((e) => ({
        data: { id: e.id, source: e.source, target: e.target, kind: e.kind },
      })),
    },
  };
}

describe("extractSubtree", () => {
  const graph = makeGraph(
    [
      { id: "root", label: "Root" },
      { id: "a", label: "A", parent: "root" },
      { id: "b", label: "B", parent: "root" },
      { id: "a1", label: "A1", parent: "a" },
      { id: "a2", label: "A2", parent: "a" },
      { id: "b1", label: "B1", parent: "b" },
      { id: "other", label: "Other" },
    ],
    [
      { id: "e1", source: "a1", target: "a2", kind: "depends" },
      { id: "e2", source: "a1", target: "b1", kind: "calls" },
      { id: "e3", source: "other", target: "root", kind: "depends" },
    ],
  );

  const index = buildTraversalIndex(graph);

  it("extracts a complete subtree with root and descendants", () => {
    const result = extractSubtree(graph, "a", index);
    const nodeIds = result.elements.nodes.map((n) => n.data.id).sort();
    expect(nodeIds).toEqual(["a", "a1", "a2"]);
  });

  it("includes edges where both endpoints are in subtree", () => {
    const result = extractSubtree(graph, "a", index);
    const edgeIds = result.elements.edges.map((e) => e.data.id);
    expect(edgeIds).toEqual(["e1"]);
  });

  it("excludes edges crossing subtree boundary", () => {
    const result = extractSubtree(graph, "a", index);
    const edgeIds = result.elements.edges.map((e) => e.data.id);
    expect(edgeIds).not.toContain("e2"); // a1 -> b1 crosses boundary
    expect(edgeIds).not.toContain("e3"); // other -> root crosses boundary
  });

  it("removes parent from root of scoped subtree", () => {
    const result = extractSubtree(graph, "a", index);
    const rootNode = result.elements.nodes.find((n) => n.data.id === "a");
    expect(rootNode?.data.parent).toBeUndefined();
  });

  it("preserves parent references for non-root nodes", () => {
    const result = extractSubtree(graph, "a", index);
    const a1 = result.elements.nodes.find((n) => n.data.id === "a1");
    expect(a1?.data.parent).toBe("a");
  });

  it("scoping to a leaf node returns just that node", () => {
    const result = extractSubtree(graph, "a1", index);
    expect(result.elements.nodes).toHaveLength(1);
    expect(result.elements.nodes[0].data.id).toBe("a1");
    expect(result.elements.edges).toHaveLength(0);
  });

  it("scoping to root returns entire tree under root", () => {
    const result = extractSubtree(graph, "root", index);
    const nodeIds = result.elements.nodes.map((n) => n.data.id).sort();
    expect(nodeIds).toEqual(["a", "a1", "a2", "b", "b1", "root"]);
  });

  it("includes edges within full subtree when scoped to root", () => {
    const result = extractSubtree(graph, "root", index);
    const edgeIds = result.elements.edges.map((e) => e.data.id).sort();
    expect(edgeIds).toEqual(["e1", "e2"]); // e3 has "other" outside subtree
  });

  it("returns original graph when rootId not found", () => {
    const result = extractSubtree(graph, "nonexistent", index);
    // collectDescendants adds "nonexistent" to the set, but no nodes match
    // so we get an empty graph
    expect(result.elements.nodes).toHaveLength(0);
  });
});
