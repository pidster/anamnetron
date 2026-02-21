import { describe, it, expect } from "vitest";
import {
  buildTraversalIndex,
  getParent,
  getFirstChild,
  getNextSibling,
  getPrevSibling,
} from "../traversal";
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
          sub_kind: "module",
          canonical_path: `/${n.id}`,
          ...(n.parent ? { parent: n.parent } : {}),
        },
      })),
      edges: edges.map((e) => ({ data: e })),
    },
  };
}

describe("buildTraversalIndex", () => {
  it("extracts parent-child relationships", () => {
    const graph = makeGraph([
      { id: "root", label: "Root" },
      { id: "child1", label: "Child 1", parent: "root" },
      { id: "child2", label: "Child 2", parent: "root" },
    ]);
    const index = buildTraversalIndex(graph);

    expect(index.parentMap.get("child1")).toBe("root");
    expect(index.parentMap.get("child2")).toBe("root");
    expect(index.parentMap.has("root")).toBe(false);
  });

  it("groups root nodes as siblings", () => {
    const graph = makeGraph([
      { id: "a", label: "Alpha" },
      { id: "b", label: "Beta" },
      { id: "c", label: "Charlie" },
    ]);
    const index = buildTraversalIndex(graph);

    expect(index.siblingsMap.get("a")).toEqual(["a", "b", "c"]);
    expect(index.siblingsMap.get("b")).toEqual(["a", "b", "c"]);
  });

  it("sorts children alphabetically by label", () => {
    const graph = makeGraph([
      { id: "parent", label: "Parent" },
      { id: "z", label: "Zeta", parent: "parent" },
      { id: "a", label: "Alpha", parent: "parent" },
      { id: "m", label: "Mid", parent: "parent" },
    ]);
    const index = buildTraversalIndex(graph);

    expect(index.childrenMap.get("parent")).toEqual(["a", "m", "z"]);
  });

  it("handles single-node graph", () => {
    const graph = makeGraph([{ id: "solo", label: "Solo" }]);
    const index = buildTraversalIndex(graph);

    expect(index.parentMap.size).toBe(0);
    expect(index.childrenMap.size).toBe(0);
    expect(index.siblingsMap.get("solo")).toEqual(["solo"]);
  });

  it("handles flat graph with no nesting", () => {
    const graph = makeGraph([
      { id: "x", label: "X" },
      { id: "y", label: "Y" },
    ]);
    const index = buildTraversalIndex(graph);

    expect(index.parentMap.size).toBe(0);
    expect(index.childrenMap.size).toBe(0);
    // Both are root siblings
    expect(index.siblingsMap.get("x")).toEqual(["x", "y"]);
  });
});

describe("getParent", () => {
  it("returns parent ID for child node", () => {
    const graph = makeGraph([
      { id: "p", label: "Parent" },
      { id: "c", label: "Child", parent: "p" },
    ]);
    const index = buildTraversalIndex(graph);

    expect(getParent(index, "c")).toBe("p");
  });

  it("returns null for root node", () => {
    const graph = makeGraph([{ id: "root", label: "Root" }]);
    const index = buildTraversalIndex(graph);

    expect(getParent(index, "root")).toBeNull();
  });
});

describe("getFirstChild", () => {
  it("returns first child alphabetically", () => {
    const graph = makeGraph([
      { id: "p", label: "Parent" },
      { id: "z", label: "Zeta", parent: "p" },
      { id: "a", label: "Alpha", parent: "p" },
    ]);
    const index = buildTraversalIndex(graph);

    expect(getFirstChild(index, "p")).toBe("a");
  });

  it("returns null for leaf node", () => {
    const graph = makeGraph([
      { id: "p", label: "Parent" },
      { id: "leaf", label: "Leaf", parent: "p" },
    ]);
    const index = buildTraversalIndex(graph);

    expect(getFirstChild(index, "leaf")).toBeNull();
  });
});

describe("getNextSibling", () => {
  it("returns next sibling in alphabetical order", () => {
    const graph = makeGraph([
      { id: "p", label: "Parent" },
      { id: "a", label: "Alpha", parent: "p" },
      { id: "b", label: "Beta", parent: "p" },
      { id: "c", label: "Charlie", parent: "p" },
    ]);
    const index = buildTraversalIndex(graph);

    expect(getNextSibling(index, "a")).toBe("b");
    expect(getNextSibling(index, "b")).toBe("c");
  });

  it("wraps around from last to first sibling", () => {
    const graph = makeGraph([
      { id: "p", label: "Parent" },
      { id: "a", label: "Alpha", parent: "p" },
      { id: "b", label: "Beta", parent: "p" },
    ]);
    const index = buildTraversalIndex(graph);

    expect(getNextSibling(index, "b")).toBe("a");
  });

  it("returns null when node has no siblings", () => {
    const graph = makeGraph([
      { id: "p", label: "Parent" },
      { id: "only", label: "Only", parent: "p" },
    ]);
    const index = buildTraversalIndex(graph);

    expect(getNextSibling(index, "only")).toBeNull();
  });
});

describe("getPrevSibling", () => {
  it("returns previous sibling in alphabetical order", () => {
    const graph = makeGraph([
      { id: "p", label: "Parent" },
      { id: "a", label: "Alpha", parent: "p" },
      { id: "b", label: "Beta", parent: "p" },
      { id: "c", label: "Charlie", parent: "p" },
    ]);
    const index = buildTraversalIndex(graph);

    expect(getPrevSibling(index, "b")).toBe("a");
    expect(getPrevSibling(index, "c")).toBe("b");
  });

  it("wraps around from first to last sibling", () => {
    const graph = makeGraph([
      { id: "p", label: "Parent" },
      { id: "a", label: "Alpha", parent: "p" },
      { id: "b", label: "Beta", parent: "p" },
    ]);
    const index = buildTraversalIndex(graph);

    expect(getPrevSibling(index, "a")).toBe("b");
  });

  it("returns null when node has no siblings", () => {
    const graph = makeGraph([
      { id: "p", label: "Parent" },
      { id: "only", label: "Only", parent: "p" },
    ]);
    const index = buildTraversalIndex(graph);

    expect(getPrevSibling(index, "only")).toBeNull();
  });
});
