import { describe, it, expect } from "vitest";
import {
  computeVisibleElements,
  computeDefaultExpansion,
  computeExpansionFromNode,
  getAncestorChain,
  countDescendants,
} from "../expansion";
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
          sub_kind: "module",
          canonical_path: `/${n.id}`,
          ...(n.parent ? { parent: n.parent } : {}),
        },
      })),
      edges: edges.map((e) => ({ data: e })),
    },
  };
}

/** 4-level hierarchy mimicking the real graph structure. */
function makeHierarchy(): CytoscapeGraph {
  return makeGraph(
    [
      { id: "sys", label: "System" },
      { id: "svc1", label: "Service 1", parent: "sys" },
      { id: "svc2", label: "Service 2", parent: "sys" },
      { id: "comp1", label: "Component 1", parent: "svc1" },
      { id: "comp2", label: "Component 2", parent: "svc1" },
      { id: "comp3", label: "Component 3", parent: "svc2" },
      { id: "unit1", label: "Unit 1", parent: "comp1" },
      { id: "unit2", label: "Unit 2", parent: "comp1" },
      { id: "unit3", label: "Unit 3", parent: "comp2" },
      { id: "unit4", label: "Unit 4", parent: "comp3" },
    ],
    [
      { id: "e1", source: "unit1", target: "unit2", kind: "depends" },
      { id: "e2", source: "comp1", target: "comp2", kind: "depends" },
      { id: "e3", source: "unit1", target: "unit3", kind: "calls" },
      { id: "e4", source: "svc1", target: "svc2", kind: "depends" },
    ],
  );
}

describe("computeDefaultExpansion", () => {
  it("depth 0 returns empty set (only roots visible)", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    const expanded = computeDefaultExpansion(index, 0);
    expect(expanded.size).toBe(0);
  });

  it("depth 1 expands only roots", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    const expanded = computeDefaultExpansion(index, 1);
    expect(expanded.has("sys")).toBe(true);
    expect(expanded.has("svc1")).toBe(false);
    expect(expanded.has("svc2")).toBe(false);
  });

  it("depth 2 expands roots and services", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    const expanded = computeDefaultExpansion(index, 2);
    expect(expanded.has("sys")).toBe(true);
    expect(expanded.has("svc1")).toBe(true);
    expect(expanded.has("svc2")).toBe(true);
    expect(expanded.has("comp1")).toBe(false);
  });

  it("depth 3 expands down to components", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    const expanded = computeDefaultExpansion(index, 3);
    expect(expanded.has("sys")).toBe(true);
    expect(expanded.has("svc1")).toBe(true);
    expect(expanded.has("comp1")).toBe(true);
    expect(expanded.has("comp2")).toBe(true);
    expect(expanded.has("comp3")).toBe(true);
    // Leaf units should not be in expanded set (they have no children)
    expect(expanded.has("unit1")).toBe(false);
  });
});

describe("computeExpansionFromNode", () => {
  it("depth 0 returns empty set", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    const expanded = computeExpansionFromNode(index, "svc1", 0);
    expect(expanded.size).toBe(0);
  });

  it("depth 1 expands only the start node", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    const expanded = computeExpansionFromNode(index, "svc1", 1);
    expect(expanded.has("svc1")).toBe(true);
    expect(expanded.has("comp1")).toBe(false);
    expect(expanded.has("sys")).toBe(false);
  });

  it("depth 2 expands start node and its children", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    const expanded = computeExpansionFromNode(index, "svc1", 2);
    expect(expanded.has("svc1")).toBe(true);
    expect(expanded.has("comp1")).toBe(true);
    expect(expanded.has("comp2")).toBe(true);
    // Leaf units should not be expanded (they're at depth 3 from svc1)
    expect(expanded.has("unit1")).toBe(false);
  });

  it("depth 3 expands down to leaf parents", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    const expanded = computeExpansionFromNode(index, "svc1", 3);
    expect(expanded.has("svc1")).toBe(true);
    expect(expanded.has("comp1")).toBe(true);
    expect(expanded.has("comp2")).toBe(true);
    // unit1, unit2, unit3 are at depth 3 — added to expanded set
    expect(expanded.has("unit1")).toBe(true);
    expect(expanded.has("unit2")).toBe(true);
    expect(expanded.has("unit3")).toBe(true);
  });

  it("does not expand nodes outside the subtree", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    const expanded = computeExpansionFromNode(index, "svc1", 10);
    expect(expanded.has("sys")).toBe(false);
    expect(expanded.has("svc2")).toBe(false);
    expect(expanded.has("comp3")).toBe(false);
    expect(expanded.has("unit4")).toBe(false);
  });
});

describe("computeVisibleElements", () => {
  it("with no expansion, only root nodes are visible", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    const result = computeVisibleElements(graph, new Set(), index);
    const ids = result.elements.nodes.map((n) => n.data.id);
    expect(ids).toEqual(["sys"]);
  });

  it("expanding root shows its direct children", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    const result = computeVisibleElements(graph, new Set(["sys"]), index);
    const ids = result.elements.nodes.map((n) => n.data.id).sort();
    expect(ids).toEqual(["svc1", "svc2", "sys"]);
  });

  it("expanding root + service shows components", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    const result = computeVisibleElements(graph, new Set(["sys", "svc1"]), index);
    const ids = result.elements.nodes.map((n) => n.data.id).sort();
    expect(ids).toEqual(["comp1", "comp2", "svc1", "svc2", "sys"]);
  });

  it("collapsed parent nodes get _childCount", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    const result = computeVisibleElements(graph, new Set(["sys"]), index);
    const svc1 = result.elements.nodes.find((n) => n.data.id === "svc1");
    // svc1 has comp1 (2 units) + comp2 (1 unit) + 2 comps = 5 descendants
    expect(svc1?.data._childCount).toBe(5);
  });

  it("expanded parent nodes do not get _childCount", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    const result = computeVisibleElements(graph, new Set(["sys", "svc1"]), index);
    const svc1 = result.elements.nodes.find((n) => n.data.id === "svc1");
    expect(svc1?.data._childCount).toBeUndefined();
  });

  it("edges between visible nodes are preserved", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    // Expand to show services — e4 (svc1 -> svc2) should be visible
    const result = computeVisibleElements(graph, new Set(["sys"]), index);
    const edgeIds = result.elements.edges.map((e) => e.data.id);
    expect(edgeIds).toContain("e4");
  });

  it("edges with hidden endpoints are excluded", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    // Only root visible — unit-level edges should be hidden
    const result = computeVisibleElements(graph, new Set(), index);
    expect(result.elements.edges).toHaveLength(0);
  });

  it("edges between partially visible endpoints are excluded", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    // Expand sys + svc1 — e3 (unit1 -> unit3) is hidden because units not visible
    const result = computeVisibleElements(graph, new Set(["sys", "svc1"]), index);
    const edgeIds = result.elements.edges.map((e) => e.data.id);
    expect(edgeIds).not.toContain("e3");
    // e2 (comp1 -> comp2) should be visible
    expect(edgeIds).toContain("e2");
  });
});

describe("meta-edge aggregation", () => {
  it("cross-cutting edges become meta-edges when endpoints are hidden", () => {
    // Custom graph where collapse produces cross-subtree meta-edges
    const graph = makeGraph(
      [
        { id: "sys", label: "System" },
        { id: "svc1", label: "Service 1", parent: "sys" },
        { id: "svc2", label: "Service 2", parent: "sys" },
        { id: "u1", label: "U1", parent: "svc1" },
        { id: "u2", label: "U2", parent: "svc2" },
      ],
      [
        { id: "e1", source: "u1", target: "u2", kind: "calls" },
      ],
    );
    const index = buildTraversalIndex(graph);
    // Expand only sys — svc1 and svc2 are visible but collapsed
    // u1 resolves to svc1, u2 resolves to svc2
    const result = computeVisibleElements(graph, new Set(["sys"]), index);
    const metaEdges = result.elements.edges.filter((e) => e.data._isMeta);
    expect(metaEdges.length).toBeGreaterThan(0);
    const crossEdge = metaEdges.find(
      (e) => e.data.source === "svc1" && e.data.target === "svc2" && e.data.kind === "calls",
    );
    expect(crossEdge).toBeDefined();
    expect(crossEdge?.data._count).toBe(1);
  });

  it("meta-edges have correct id format", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    const result = computeVisibleElements(graph, new Set(["sys", "svc1"]), index);
    const metaEdges = result.elements.edges.filter((e) => e.data._isMeta);
    for (const edge of metaEdges) {
      expect(edge.data.id).toMatch(/^meta:/);
    }
  });

  it("edges internal to same collapsed subtree are dropped", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    // Only root expanded — e1 (unit1 -> unit2) both inside comp1 inside svc1
    // Both resolve to svc1 (same) -> dropped
    const result = computeVisibleElements(graph, new Set(["sys"]), index);
    const metaEdges = result.elements.edges.filter((e) => e.data._isMeta);
    const internal = metaEdges.find(
      (e) => e.data.source === e.data.target,
    );
    expect(internal).toBeUndefined();
  });

  it("multiple edges of same kind between same ancestors accumulate count", () => {
    // Two edges from different units in comp1 to different units in comp3
    const graph = makeGraph(
      [
        { id: "sys", label: "System" },
        { id: "svc1", label: "Service 1", parent: "sys" },
        { id: "svc2", label: "Service 2", parent: "sys" },
        { id: "u1", label: "U1", parent: "svc1" },
        { id: "u2", label: "U2", parent: "svc1" },
        { id: "u3", label: "U3", parent: "svc2" },
        { id: "u4", label: "U4", parent: "svc2" },
      ],
      [
        { id: "e1", source: "u1", target: "u3", kind: "depends" },
        { id: "e2", source: "u2", target: "u4", kind: "depends" },
      ],
    );
    const index = buildTraversalIndex(graph);
    // Expand only root — u1/u2 resolve to svc1, u3/u4 resolve to svc2
    const result = computeVisibleElements(graph, new Set(["sys"]), index);
    const metaEdges = result.elements.edges.filter((e) => e.data._isMeta);
    const cross = metaEdges.find(
      (e) => e.data.source === "svc1" && e.data.target === "svc2" && e.data.kind === "depends",
    );
    expect(cross).toBeDefined();
    expect(cross?.data._count).toBe(2);
  });

  it("different edge kinds produce separate meta-edges", () => {
    const graph = makeGraph(
      [
        { id: "sys", label: "System" },
        { id: "svc1", label: "Service 1", parent: "sys" },
        { id: "svc2", label: "Service 2", parent: "sys" },
        { id: "u1", label: "U1", parent: "svc1" },
        { id: "u2", label: "U2", parent: "svc2" },
      ],
      [
        { id: "e1", source: "u1", target: "u2", kind: "depends" },
        { id: "e2", source: "u1", target: "u2", kind: "calls" },
      ],
    );
    const index = buildTraversalIndex(graph);
    const result = computeVisibleElements(graph, new Set(["sys"]), index);
    const metaEdges = result.elements.edges.filter((e) => e.data._isMeta);
    expect(metaEdges).toHaveLength(2);
    const kinds = metaEdges.map((e) => e.data.kind).sort();
    expect(kinds).toEqual(["calls", "depends"]);
  });

  it("fully expanded graph has no meta-edges", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    const expanded = new Set(["sys", "svc1", "svc2", "comp1", "comp2", "comp3"]);
    const result = computeVisibleElements(graph, expanded, index);
    const metaEdges = result.elements.edges.filter((e) => e.data._isMeta);
    expect(metaEdges).toHaveLength(0);
  });
});

describe("getAncestorChain", () => {
  it("returns empty array for root node", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    expect(getAncestorChain(index, "sys")).toEqual([]);
  });

  it("returns correct chain for deep node", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    expect(getAncestorChain(index, "unit1")).toEqual(["sys", "svc1", "comp1"]);
  });

  it("returns single parent for depth-1 node", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    expect(getAncestorChain(index, "svc1")).toEqual(["sys"]);
  });
});

describe("countDescendants", () => {
  it("returns 0 for leaf nodes", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    expect(countDescendants(index, "unit1")).toBe(0);
  });

  it("counts direct children", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    // comp1 has unit1, unit2
    expect(countDescendants(index, "comp1")).toBe(2);
  });

  it("counts all descendants recursively", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    // svc1 -> comp1 (unit1, unit2), comp2 (unit3) = 5 descendants
    expect(countDescendants(index, "svc1")).toBe(5);
  });

  it("counts entire tree from root", () => {
    const graph = makeHierarchy();
    const index = buildTraversalIndex(graph);
    // sys -> svc1 (comp1, comp2, unit1, unit2, unit3), svc2 (comp3, unit4) = 9
    expect(countDescendants(index, "sys")).toBe(9);
  });
});

describe("edge cases", () => {
  it("single-node graph with no expansion", () => {
    const graph = makeGraph([{ id: "solo", label: "Solo" }]);
    const index = buildTraversalIndex(graph);
    const result = computeVisibleElements(graph, new Set(), index);
    expect(result.elements.nodes).toHaveLength(1);
    expect(result.elements.nodes[0].data.id).toBe("solo");
    // No children, so no _childCount
    expect(result.elements.nodes[0].data._childCount).toBeUndefined();
  });

  it("flat graph (no nesting) shows all nodes regardless of expansion", () => {
    const graph = makeGraph([
      { id: "a", label: "A" },
      { id: "b", label: "B" },
      { id: "c", label: "C" },
    ]);
    const index = buildTraversalIndex(graph);
    const result = computeVisibleElements(graph, new Set(), index);
    expect(result.elements.nodes).toHaveLength(3);
  });

  it("deep hierarchy with selective expansion", () => {
    const graph = makeGraph([
      { id: "r", label: "Root" },
      { id: "a", label: "A", parent: "r" },
      { id: "b", label: "B", parent: "a" },
      { id: "c", label: "C", parent: "b" },
      { id: "d", label: "D", parent: "c" },
    ]);
    const index = buildTraversalIndex(graph);
    // Expand r and a, but not b
    const result = computeVisibleElements(graph, new Set(["r", "a"]), index);
    const ids = result.elements.nodes.map((n) => n.data.id).sort();
    expect(ids).toEqual(["a", "b", "r"]);
    // b is collapsed — should have _childCount
    const bNode = result.elements.nodes.find((n) => n.data.id === "b");
    expect(bNode?.data._childCount).toBe(2); // c and d
  });
});
