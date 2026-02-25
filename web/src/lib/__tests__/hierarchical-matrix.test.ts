import { describe, it, expect } from "vitest";
import { buildHierarchicalMatrix } from "../hierarchical-matrix";
import type { CytoscapeGraph } from "../types";

function makeGraph(
  nodes: Array<{ id: string; label: string; kind?: string; parent?: string }>,
  edges: Array<{
    id: string;
    source: string;
    target: string;
    kind: string;
  }> = [],
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

describe("buildHierarchicalMatrix", () => {
  it("returns empty result for empty graph", () => {
    const graph = makeGraph([]);
    const result = buildHierarchicalMatrix(graph, new Set());
    expect(result.nodes).toEqual([]);
    expect(result.cells).toEqual([]);
    expect(result.maxCount).toBe(0);
  });

  it("shows all nodes when fully expanded", () => {
    const graph = makeGraph([
      { id: "root", label: "Root", kind: "system" },
      { id: "svc-a", label: "Service A", parent: "root" },
      { id: "a1", label: "A1", parent: "svc-a" },
      { id: "a2", label: "A2", parent: "svc-a" },
    ]);

    const expanded = new Set(["root", "svc-a"]);
    const result = buildHierarchicalMatrix(graph, expanded);

    expect(result.nodes).toHaveLength(4);
    expect(result.nodes.map((n) => n.id)).toEqual([
      "root",
      "svc-a",
      "a1",
      "a2",
    ]);
  });

  it("collapses children when parent not expanded", () => {
    const graph = makeGraph([
      { id: "root", label: "Root", kind: "system" },
      { id: "svc-a", label: "Service A", parent: "root" },
      { id: "a1", label: "A1", parent: "svc-a" },
      { id: "a2", label: "A2", parent: "svc-a" },
    ]);

    // Only root expanded — svc-a visible but its children hidden
    const expanded = new Set(["root"]);
    const result = buildHierarchicalMatrix(graph, expanded);

    expect(result.nodes).toHaveLength(2);
    expect(result.nodes.map((n) => n.id)).toEqual(["root", "svc-a"]);
    // svc-a should show as having children but not expanded
    expect(result.nodes[1].hasChildren).toBe(true);
    expect(result.nodes[1].expanded).toBe(false);
  });

  it("aggregates edges to nearest visible ancestor when collapsed", () => {
    const graph = makeGraph(
      [
        { id: "root", label: "Root", kind: "system" },
        { id: "svc-a", label: "A", parent: "root" },
        { id: "a1", label: "A1", parent: "svc-a" },
        { id: "svc-b", label: "B", parent: "root" },
        { id: "b1", label: "B1", parent: "svc-b" },
      ],
      [{ id: "e1", source: "a1", target: "b1", kind: "depends" }],
    );

    // Root expanded, services collapsed
    const expanded = new Set(["root"]);
    const result = buildHierarchicalMatrix(graph, expanded);

    // Should have edge from svc-a to svc-b
    expect(result.cells).toHaveLength(1);
    const aIdx = result.nodes.findIndex((n) => n.id === "svc-a");
    const bIdx = result.nodes.findIndex((n) => n.id === "svc-b");
    expect(result.cells[0].row).toBe(aIdx);
    expect(result.cells[0].col).toBe(bIdx);
    expect(result.cells[0].count).toBe(1);
  });

  it("splits rows and columns when expanding a node", () => {
    const graph = makeGraph(
      [
        { id: "root", label: "Root", kind: "system" },
        { id: "svc-a", label: "A", parent: "root" },
        { id: "a1", label: "A1", parent: "svc-a" },
        { id: "a2", label: "A2", parent: "svc-a" },
        { id: "svc-b", label: "B", parent: "root" },
        { id: "b1", label: "B1", parent: "svc-b" },
      ],
      [
        { id: "e1", source: "a1", target: "b1", kind: "depends" },
        { id: "e2", source: "a2", target: "b1", kind: "depends" },
      ],
    );

    // Collapsed: 1 cell from svc-a -> svc-b with count 2
    const collapsed = buildHierarchicalMatrix(
      graph,
      new Set(["root"]),
    );
    expect(collapsed.cells).toHaveLength(1);
    expect(collapsed.cells[0].count).toBe(2);

    // Expanded: 2 cells (a1->svc-b, a2->svc-b) each with count 1
    const expanded = buildHierarchicalMatrix(
      graph,
      new Set(["root", "svc-a"]),
    );
    const bIdx = expanded.nodes.findIndex((n) => n.id === "svc-b");
    const bCells = expanded.cells.filter((c) => c.col === bIdx);
    expect(bCells).toHaveLength(2);
    expect(bCells.every((c) => c.count === 1)).toBe(true);
  });

  it("ignores contains edges", () => {
    const graph = makeGraph(
      [
        { id: "root", label: "Root" },
        { id: "svc-a", label: "A", parent: "root" },
        { id: "svc-b", label: "B", parent: "root" },
      ],
      [{ id: "c1", source: "root", target: "svc-a", kind: "contains" }],
    );

    const result = buildHierarchicalMatrix(graph, new Set(["root"]));
    expect(result.cells).toEqual([]);
  });

  it("ignores self-edges after aggregation", () => {
    const graph = makeGraph(
      [
        { id: "root", label: "Root" },
        { id: "svc-a", label: "A", parent: "root" },
        { id: "a1", label: "A1", parent: "svc-a" },
        { id: "a2", label: "A2", parent: "svc-a" },
      ],
      [{ id: "e1", source: "a1", target: "a2", kind: "depends" }],
    );

    // Collapsed: a1 and a2 both aggregate to svc-a, making it a self-edge
    const result = buildHierarchicalMatrix(graph, new Set(["root"]));
    expect(result.cells).toEqual([]);
  });

  it("detects cycles (bidirectional edges)", () => {
    const graph = makeGraph(
      [
        { id: "root", label: "Root" },
        { id: "svc-a", label: "A", parent: "root" },
        { id: "svc-b", label: "B", parent: "root" },
      ],
      [
        { id: "e1", source: "svc-a", target: "svc-b", kind: "depends" },
        { id: "e2", source: "svc-b", target: "svc-a", kind: "depends" },
      ],
    );

    const result = buildHierarchicalMatrix(graph, new Set(["root"]));
    expect(result.cells).toHaveLength(2);
    expect(result.cells.every((c) => c.isCyclic)).toBe(true);
  });

  it("tracks maximum count", () => {
    const graph = makeGraph(
      [
        { id: "root", label: "Root" },
        { id: "svc-a", label: "A", parent: "root" },
        { id: "a1", label: "A1", parent: "svc-a" },
        { id: "a2", label: "A2", parent: "svc-a" },
        { id: "svc-b", label: "B", parent: "root" },
        { id: "b1", label: "B1", parent: "svc-b" },
      ],
      [
        { id: "e1", source: "a1", target: "b1", kind: "depends" },
        { id: "e2", source: "a2", target: "b1", kind: "calls" },
      ],
    );

    const result = buildHierarchicalMatrix(graph, new Set(["root"]));
    expect(result.maxCount).toBe(2);
  });

  it("supports alphabetical sort mode", () => {
    const graph = makeGraph([
      { id: "root", label: "Root" },
      { id: "c", label: "Charlie", parent: "root" },
      { id: "a", label: "Alpha", parent: "root" },
      { id: "b", label: "Bravo", parent: "root" },
    ]);

    const result = buildHierarchicalMatrix(
      graph,
      new Set(["root"]),
      "alphabetical",
    );
    const labels = result.nodes.slice(1).map((n) => n.label);
    expect(labels).toEqual(["Alpha", "Bravo", "Charlie"]);
  });

  it("supports dependency-count sort mode", () => {
    const graph = makeGraph(
      [
        { id: "root", label: "Root" },
        { id: "svc-a", label: "A", parent: "root" },
        { id: "svc-b", label: "B", parent: "root" },
        { id: "svc-c", label: "C", parent: "root" },
      ],
      [
        { id: "e1", source: "svc-a", target: "svc-b", kind: "depends" },
        { id: "e2", source: "svc-a", target: "svc-c", kind: "depends" },
        { id: "e3", source: "svc-b", target: "svc-c", kind: "depends" },
      ],
    );

    const result = buildHierarchicalMatrix(
      graph,
      new Set(["root"]),
      "dependency-count",
    );

    // All nodes should still be present
    expect(result.nodes).toHaveLength(4);
    // Cells should be remapped correctly
    expect(result.cells.length).toBeGreaterThan(0);
  });

  it("correctly records depth for nested nodes", () => {
    const graph = makeGraph([
      { id: "root", label: "Root" },
      { id: "svc", label: "Service", parent: "root" },
      { id: "comp", label: "Component", parent: "svc" },
      { id: "unit", label: "Unit", parent: "comp" },
    ]);

    const result = buildHierarchicalMatrix(
      graph,
      new Set(["root", "svc", "comp"]),
    );

    expect(result.nodes.find((n) => n.id === "root")?.depth).toBe(0);
    expect(result.nodes.find((n) => n.id === "svc")?.depth).toBe(1);
    expect(result.nodes.find((n) => n.id === "comp")?.depth).toBe(2);
    expect(result.nodes.find((n) => n.id === "unit")?.depth).toBe(3);
  });
});
