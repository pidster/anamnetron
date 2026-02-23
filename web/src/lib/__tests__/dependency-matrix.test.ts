import { describe, it, expect } from "vitest";
import { buildDependencyMatrix } from "../dependency-matrix";
import type { CytoscapeGraph } from "../types";

function makeGraph(
  nodes: Array<{ id: string; label: string; kind?: string; parent?: string }>,
  edges: Array<{ id: string; source: string; target: string; kind: string }> = [],
): CytoscapeGraph {
  return {
    elements: {
      nodes: nodes.map((n) => ({
        data: {
          id: n.id,
          label: n.label,
          kind: n.kind ?? "component",
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

describe("buildDependencyMatrix", () => {
  it("returns empty matrix when graph has no nodes", () => {
    const graph = makeGraph([], []);
    const result = buildDependencyMatrix(graph);
    expect(result.names).toEqual([]);
    expect(result.ids).toEqual([]);
    expect(result.matrix).toEqual([]);
  });

  it("identifies top-level modules as depth-1 nodes", () => {
    const graph = makeGraph([
      { id: "root", label: "System", kind: "system" },
      { id: "svc-a", label: "Service A", kind: "service", parent: "root" },
      { id: "svc-b", label: "Service B", kind: "service", parent: "root" },
      { id: "comp-a1", label: "Component A1", parent: "svc-a" },
      { id: "comp-b1", label: "Component B1", parent: "svc-b" },
    ]);

    const result = buildDependencyMatrix(graph);
    expect(result.ids.sort()).toEqual(["svc-a", "svc-b"]);
    expect(result.names.sort()).toEqual(["Service A", "Service B"]);
  });

  it("aggregates dependency edges between top-level modules", () => {
    const graph = makeGraph(
      [
        { id: "root", label: "System" },
        { id: "svc-a", label: "A", parent: "root" },
        { id: "svc-b", label: "B", parent: "root" },
        { id: "a1", label: "A1", parent: "svc-a" },
        { id: "a2", label: "A2", parent: "svc-a" },
        { id: "b1", label: "B1", parent: "svc-b" },
      ],
      [
        { id: "e1", source: "a1", target: "b1", kind: "depends" },
        { id: "e2", source: "a2", target: "b1", kind: "calls" },
        { id: "e3", source: "b1", target: "a1", kind: "depends" },
      ],
    );

    const result = buildDependencyMatrix(graph);
    const aIdx = result.ids.indexOf("svc-a");
    const bIdx = result.ids.indexOf("svc-b");

    // 2 edges from A -> B (e1, e2)
    expect(result.matrix[aIdx][bIdx]).toBe(2);
    // 1 edge from B -> A (e3)
    expect(result.matrix[bIdx][aIdx]).toBe(1);
  });

  it("ignores contains edges", () => {
    const graph = makeGraph(
      [
        { id: "root", label: "System" },
        { id: "svc-a", label: "A", parent: "root" },
        { id: "svc-b", label: "B", parent: "root" },
        { id: "a1", label: "A1", parent: "svc-a" },
      ],
      [
        { id: "c1", source: "svc-a", target: "a1", kind: "contains" },
      ],
    );

    const result = buildDependencyMatrix(graph);
    // Matrix should be all zeros — contains edges are excluded
    for (const row of result.matrix) {
      for (const val of row) {
        expect(val).toBe(0);
      }
    }
  });

  it("ignores self-module edges", () => {
    const graph = makeGraph(
      [
        { id: "root", label: "System" },
        { id: "svc-a", label: "A", parent: "root" },
        { id: "a1", label: "A1", parent: "svc-a" },
        { id: "a2", label: "A2", parent: "svc-a" },
      ],
      [
        { id: "e1", source: "a1", target: "a2", kind: "depends" },
      ],
    );

    const result = buildDependencyMatrix(graph);
    // Intra-module edges should not appear in the matrix
    expect(result.matrix[0][0]).toBe(0);
  });

  it("assigns categorical colours", () => {
    const graph = makeGraph([
      { id: "root", label: "System" },
      { id: "svc-a", label: "A", parent: "root" },
      { id: "svc-b", label: "B", parent: "root" },
    ]);

    const result = buildDependencyMatrix(graph);
    expect(result.colors).toHaveLength(2);
    // Colours should be from schemeTableau10
    for (const c of result.colors) {
      expect(c).toMatch(/^#[0-9a-f]{6}$/i);
    }
  });

  it("aggregates deeply nested nodes to their top-level ancestor", () => {
    const graph = makeGraph(
      [
        { id: "root", label: "System" },
        { id: "svc-a", label: "A", parent: "root" },
        { id: "svc-b", label: "B", parent: "root" },
        { id: "comp-a", label: "CA", parent: "svc-a" },
        { id: "unit-a", label: "UA", parent: "comp-a" },
        { id: "comp-b", label: "CB", parent: "svc-b" },
        { id: "unit-b", label: "UB", parent: "comp-b" },
      ],
      [
        { id: "e1", source: "unit-a", target: "unit-b", kind: "depends" },
      ],
    );

    const result = buildDependencyMatrix(graph);
    const aIdx = result.ids.indexOf("svc-a");
    const bIdx = result.ids.indexOf("svc-b");

    expect(result.matrix[aIdx][bIdx]).toBe(1);
    expect(result.matrix[bIdx][aIdx]).toBe(0);
  });

  it("handles graph with only root nodes (no top-level modules)", () => {
    const graph = makeGraph([
      { id: "root1", label: "Root1" },
      { id: "root2", label: "Root2" },
    ]);

    const result = buildDependencyMatrix(graph);
    // Root nodes have no parent, so they are not top-level modules
    expect(result.names).toEqual([]);
    expect(result.matrix).toEqual([]);
  });
});
