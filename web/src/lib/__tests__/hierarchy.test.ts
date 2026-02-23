import { describe, it, expect } from "vitest";
import { buildHierarchy, sumByMetric, getMetric, type TreeNode } from "../hierarchy";
import type { CytoscapeGraph } from "../types";

function makeGraph(
  nodes: Array<{
    id: string;
    label: string;
    parent?: string;
    metadata?: Record<string, unknown>;
  }>,
): CytoscapeGraph {
  return {
    elements: {
      nodes: nodes.map((n) => ({
        data: {
          id: n.id,
          label: n.label,
          kind: "component",
          sub_kind: "module",
          canonical_path: `/${n.id}`,
          parent: n.parent,
          metadata: n.metadata,
        },
      })),
      edges: [],
    },
  };
}

describe("buildHierarchy", () => {
  it("builds a single-root tree from parent references", () => {
    const graph = makeGraph([
      { id: "root", label: "Root" },
      { id: "a", label: "A", parent: "root" },
      { id: "b", label: "B", parent: "root" },
    ]);

    const root = buildHierarchy(graph);
    expect(root.data.id).toBe("root");
    expect(root.children).toHaveLength(2);
    expect(root.children!.map((c) => c.data.id).sort()).toEqual(["a", "b"]);
  });

  it("creates synthetic root when multiple top-level nodes exist", () => {
    const graph = makeGraph([
      { id: "a", label: "A" },
      { id: "b", label: "B" },
    ]);

    const root = buildHierarchy(graph);
    expect(root.data.id).toBe("__root__");
    expect(root.children).toHaveLength(2);
  });

  it("uses single node as root without synthetic wrapper", () => {
    const graph = makeGraph([{ id: "only", label: "Only" }]);

    const root = buildHierarchy(graph);
    expect(root.data.id).toBe("only");
  });

  it("returns unsummed hierarchy (no .value set)", () => {
    const graph = makeGraph([
      { id: "root", label: "Root" },
      { id: "a", label: "A", parent: "root", metadata: { loc: 100 } },
    ]);

    const root = buildHierarchy(graph);
    expect(root.value).toBeUndefined();
  });

  it("builds deep nesting correctly", () => {
    const graph = makeGraph([
      { id: "root", label: "Root" },
      { id: "a", label: "A", parent: "root" },
      { id: "a1", label: "A1", parent: "a", metadata: { loc: 50 } },
      { id: "a2", label: "A2", parent: "a", metadata: { loc: 75 } },
    ]);

    const root = buildHierarchy(graph);
    const aNode = root.children!.find((c) => c.data.id === "a");
    expect(aNode).toBeDefined();
    expect(aNode!.children).toHaveLength(2);
  });

  it("preserves metadata on tree nodes", () => {
    const graph = makeGraph([
      { id: "root", label: "Root", metadata: { fan_in: 3, fan_out: 5 } },
    ]);

    const root = buildHierarchy(graph);
    expect(root.data.metadata).toEqual({ fan_in: 3, fan_out: 5 });
  });

  it("handles empty graph", () => {
    const graph: CytoscapeGraph = { elements: { nodes: [], edges: [] } };
    const root = buildHierarchy(graph);
    // Synthetic root with no children
    expect(root.data.id).toBe("__root__");
    expect(root.children).toBeUndefined();
  });
});

describe("getMetric", () => {
  it("returns numeric value from metadata", () => {
    const node: TreeNode = {
      id: "a",
      label: "A",
      kind: "component",
      sub_kind: "module",
      canonical_path: "/a",
      metadata: { loc: 42, fan_out: 7 },
      children: [],
    };
    expect(getMetric(node, "loc")).toBe(42);
    expect(getMetric(node, "fan_out")).toBe(7);
  });

  it("returns 0 for missing metadata", () => {
    const node: TreeNode = {
      id: "a",
      label: "A",
      kind: "component",
      sub_kind: "module",
      canonical_path: "/a",
      children: [],
    };
    expect(getMetric(node, "loc")).toBe(0);
  });

  it("returns 0 for non-numeric metadata values", () => {
    const node: TreeNode = {
      id: "a",
      label: "A",
      kind: "component",
      sub_kind: "module",
      canonical_path: "/a",
      metadata: { loc: "not a number" },
      children: [],
    };
    expect(getMetric(node, "loc")).toBe(0);
  });

  it("returns 0 for missing key in metadata", () => {
    const node: TreeNode = {
      id: "a",
      label: "A",
      kind: "component",
      sub_kind: "module",
      canonical_path: "/a",
      metadata: { other: 10 },
      children: [],
    };
    expect(getMetric(node, "loc")).toBe(0);
  });
});

describe("sumByMetric", () => {
  it("sums LOC from metadata for leaf sizing", () => {
    const graph = makeGraph([
      { id: "root", label: "Root" },
      { id: "a", label: "A", parent: "root", metadata: { loc: 100 } },
      { id: "b", label: "B", parent: "root", metadata: { loc: 200 } },
    ]);

    const root = sumByMetric(buildHierarchy(graph), "loc");
    expect(root.value).toBe(300);
  });

  it("defaults leaf value to 1 when metric is missing", () => {
    const graph = makeGraph([
      { id: "root", label: "Root" },
      { id: "a", label: "A", parent: "root" },
    ]);

    const root = sumByMetric(buildHierarchy(graph), "loc");
    expect(root.value).toBe(1);
  });

  it("sums deep nesting correctly", () => {
    const graph = makeGraph([
      { id: "root", label: "Root" },
      { id: "a", label: "A", parent: "root" },
      { id: "a1", label: "A1", parent: "a", metadata: { loc: 50 } },
      { id: "a2", label: "A2", parent: "a", metadata: { loc: 75 } },
    ]);

    const root = sumByMetric(buildHierarchy(graph), "loc");
    expect(root.value).toBe(125);
    const aNode = root.children!.find((c) => c.data.id === "a");
    expect(aNode!.value).toBe(125);
  });

  it("uses count metric to give every leaf value 1", () => {
    const graph = makeGraph([
      { id: "root", label: "Root" },
      { id: "a", label: "A", parent: "root", metadata: { loc: 500 } },
      { id: "b", label: "B", parent: "root", metadata: { loc: 10 } },
      { id: "c", label: "C", parent: "root" },
    ]);

    const root = sumByMetric(buildHierarchy(graph), "count");
    expect(root.value).toBe(3);
  });

  it("sums fan_in metric", () => {
    const graph = makeGraph([
      { id: "root", label: "Root" },
      { id: "a", label: "A", parent: "root", metadata: { fan_in: 5 } },
      { id: "b", label: "B", parent: "root", metadata: { fan_in: 3 } },
    ]);

    const root = sumByMetric(buildHierarchy(graph), "fan_in");
    expect(root.value).toBe(8);
  });
});
