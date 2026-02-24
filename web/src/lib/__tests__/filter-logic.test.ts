import { describe, it, expect } from "vitest";
import {
  toggleInSet,
  extractSubKinds,
  extractLanguages,
  hasActiveFilters,
  formatLabel,
  filterGraph,
} from "../filter-logic";
import type { FilterState } from "../filter-logic";
import type { CyNodeData, CytoscapeGraph } from "../types";

function makeNode(overrides: Partial<CyNodeData> = {}): { data: CyNodeData } {
  return {
    data: {
      id: "n1",
      label: "Node 1",
      kind: "component",
      sub_kind: "module",
      canonical_path: "/n1",
      ...overrides,
    },
  };
}

describe("toggleInSet", () => {
  it("adds item when not present", () => {
    const set = new Set(["a", "b"]);
    const result = toggleInSet(set, "c");
    expect(result).toEqual(new Set(["a", "b", "c"]));
  });

  it("removes item when present", () => {
    const set = new Set(["a", "b", "c"]);
    const result = toggleInSet(set, "b");
    expect(result).toEqual(new Set(["a", "c"]));
  });

  it("does not mutate the original set", () => {
    const set = new Set(["a"]);
    toggleInSet(set, "b");
    expect(set).toEqual(new Set(["a"]));
  });
});

describe("extractSubKinds", () => {
  it("extracts unique sorted sub-kinds", () => {
    const nodes = [
      makeNode({ sub_kind: "function" }),
      makeNode({ sub_kind: "module" }),
      makeNode({ sub_kind: "function" }),
      makeNode({ sub_kind: "crate" }),
    ];
    expect(extractSubKinds(nodes)).toEqual(["crate", "function", "module"]);
  });

  it("returns empty array for no nodes", () => {
    expect(extractSubKinds([])).toEqual([]);
  });
});

describe("extractLanguages", () => {
  it("extracts unique sorted languages", () => {
    const nodes = [
      makeNode({ language: "typescript" }),
      makeNode({ language: "rust" }),
      makeNode({ language: "rust" }),
    ];
    expect(extractLanguages(nodes)).toEqual(["rust", "typescript"]);
  });

  it("skips nodes without language", () => {
    const nodes = [
      makeNode({ language: "rust" }),
      makeNode({ language: undefined }),
    ];
    expect(extractLanguages(nodes)).toEqual(["rust"]);
  });
});

describe("hasActiveFilters", () => {
  it("returns false when all filters are at full capacity", () => {
    const state: FilterState = {
      nodeKinds: new Set(["a", "b"]),
      edgeKinds: new Set(["c"]),
      subKinds: new Set(["d", "e"]),
      languages: new Set(["f"]),
      allNodeKinds: 2,
      allEdgeKinds: 1,
      allSubKinds: 2,
      allLanguages: 1,
      testVisibility: "all",
    };
    expect(hasActiveFilters(state)).toBe(false);
  });

  it("returns true when a node kind is filtered out", () => {
    const state: FilterState = {
      nodeKinds: new Set(["a"]),
      edgeKinds: new Set(["c"]),
      subKinds: new Set(["d", "e"]),
      languages: new Set(["f"]),
      allNodeKinds: 2,
      allEdgeKinds: 1,
      allSubKinds: 2,
      allLanguages: 1,
      testVisibility: "all",
    };
    expect(hasActiveFilters(state)).toBe(true);
  });

  it("returns true when a language is filtered out", () => {
    const state: FilterState = {
      nodeKinds: new Set(["a", "b"]),
      edgeKinds: new Set(["c"]),
      subKinds: new Set(["d", "e"]),
      languages: new Set<string>(),
      allNodeKinds: 2,
      allEdgeKinds: 1,
      allSubKinds: 2,
      allLanguages: 1,
      testVisibility: "all",
    };
    expect(hasActiveFilters(state)).toBe(true);
  });

  it("returns true when testVisibility is not all", () => {
    const state: FilterState = {
      nodeKinds: new Set(["a", "b"]),
      edgeKinds: new Set(["c"]),
      subKinds: new Set(["d", "e"]),
      languages: new Set(["f"]),
      allNodeKinds: 2,
      allEdgeKinds: 1,
      allSubKinds: 2,
      allLanguages: 1,
      testVisibility: "code",
    };
    expect(hasActiveFilters(state)).toBe(true);
  });
});

describe("formatLabel", () => {
  it("converts snake_case to Title Case", () => {
    expect(formatLabel("data_flow")).toBe("Data Flow");
  });

  it("capitalizes single words", () => {
    expect(formatLabel("system")).toBe("System");
  });
});

describe("filterGraph", () => {
  function buildGraph(): CytoscapeGraph {
    return {
      elements: {
        nodes: [
          { data: { id: "sys", label: "System", kind: "system", sub_kind: "workspace", canonical_path: "/sys" } },
          { data: { id: "svc1", label: "Service A", kind: "service", sub_kind: "crate", canonical_path: "/svc1", parent: "sys", language: "rust" } },
          { data: { id: "svc2", label: "Service B", kind: "service", sub_kind: "crate", canonical_path: "/svc2", parent: "sys", language: "typescript" } },
          { data: { id: "comp1", label: "Comp 1", kind: "component", sub_kind: "module", canonical_path: "/comp1", parent: "svc1", language: "rust" } },
          { data: { id: "unit1", label: "Unit 1", kind: "unit", sub_kind: "function", canonical_path: "/unit1", parent: "comp1", language: "rust" } },
        ],
        edges: [
          { data: { id: "e1", source: "sys", target: "svc1", kind: "contains" } },
          { data: { id: "e2", source: "sys", target: "svc2", kind: "contains" } },
          { data: { id: "e3", source: "svc1", target: "comp1", kind: "contains" } },
          { data: { id: "e4", source: "comp1", target: "unit1", kind: "contains" } },
          { data: { id: "e5", source: "svc1", target: "svc2", kind: "depends" } },
          { data: { id: "e6", source: "comp1", target: "svc2", kind: "calls" } },
        ],
      },
    };
  }

  const allKinds = new Set(["system", "service", "component", "unit"]);
  const allEdgeKinds = new Set(["depends", "calls", "implements", "extends", "data_flow", "exports"]);
  const allSubKinds = new Set(["workspace", "crate", "module", "function"]);
  const allLanguages = new Set(["rust", "typescript"]);

  it("returns all nodes and edges when no filters are active", () => {
    const graph = buildGraph();
    const result = filterGraph(graph, allKinds, allEdgeKinds, allSubKinds, allLanguages);
    expect(result.elements.nodes).toHaveLength(5);
    expect(result.elements.edges).toHaveLength(6);
  });

  it("filters by node kind and preserves ancestors", () => {
    const graph = buildGraph();
    // Only keep "unit" kind nodes
    const result = filterGraph(graph, new Set(["unit"]), allEdgeKinds, allSubKinds, allLanguages);
    const ids = result.elements.nodes.map((n) => n.data.id);
    // unit1 passes, plus ancestors: comp1, svc1, sys
    expect(ids).toContain("unit1");
    expect(ids).toContain("comp1");
    expect(ids).toContain("svc1");
    expect(ids).toContain("sys");
    // svc2 has no unit descendants, should be removed
    expect(ids).not.toContain("svc2");
  });

  it("filters by language", () => {
    const graph = buildGraph();
    // Only keep rust
    const result = filterGraph(graph, allKinds, allEdgeKinds, allSubKinds, new Set(["rust"]));
    const ids = result.elements.nodes.map((n) => n.data.id);
    expect(ids).toContain("svc1");
    expect(ids).toContain("comp1");
    expect(ids).toContain("unit1");
    // svc2 is typescript, should be removed
    expect(ids).not.toContain("svc2");
  });

  it("removes edges where an endpoint is filtered out", () => {
    const graph = buildGraph();
    const result = filterGraph(graph, allKinds, allEdgeKinds, allSubKinds, new Set(["rust"]));
    const edgeIds = result.elements.edges.map((e) => e.data.id);
    // e5 (svc1->svc2) and e6 (comp1->svc2) should be removed since svc2 is gone
    expect(edgeIds).not.toContain("e5");
    expect(edgeIds).not.toContain("e6");
    // contains edges within rust subtree remain
    expect(edgeIds).toContain("e1");
    expect(edgeIds).toContain("e3");
    expect(edgeIds).toContain("e4");
  });

  it("filters edges by edgeKinds but always keeps contains", () => {
    const graph = buildGraph();
    // Only allow "depends" edges (no "calls")
    const result = filterGraph(graph, allKinds, new Set(["depends"]), allSubKinds, allLanguages);
    const edgeIds = result.elements.edges.map((e) => e.data.id);
    // Contains edges always survive
    expect(edgeIds).toContain("e1");
    expect(edgeIds).toContain("e2");
    expect(edgeIds).toContain("e3");
    expect(edgeIds).toContain("e4");
    // depends survives
    expect(edgeIds).toContain("e5");
    // calls is filtered out
    expect(edgeIds).not.toContain("e6");
  });

  it("filters by sub_kind", () => {
    const graph = buildGraph();
    // Remove "function" sub_kind
    const result = filterGraph(graph, allKinds, allEdgeKinds, new Set(["workspace", "crate", "module"]), allLanguages);
    const ids = result.elements.nodes.map((n) => n.data.id);
    expect(ids).not.toContain("unit1");
    expect(ids).toContain("comp1");
  });

  describe("test visibility filtering", () => {
    function buildGraphWithTests(): CytoscapeGraph {
      return {
        elements: {
          nodes: [
            { data: { id: "sys", label: "System", kind: "system", sub_kind: "workspace", canonical_path: "/sys" } },
            { data: { id: "svc1", label: "Service A", kind: "service", sub_kind: "crate", canonical_path: "/svc1", parent: "sys", language: "rust" } },
            { data: { id: "comp1", label: "Comp 1", kind: "component", sub_kind: "module", canonical_path: "/comp1", parent: "svc1", language: "rust" } },
            { data: { id: "unit1", label: "Unit 1", kind: "unit", sub_kind: "function", canonical_path: "/unit1", parent: "comp1", language: "rust" } },
            { data: { id: "test1", label: "Test 1", kind: "unit", sub_kind: "function", canonical_path: "/test1", parent: "comp1", language: "rust", metadata: { tags: ["test"] } } },
          ],
          edges: [
            { data: { id: "e1", source: "sys", target: "svc1", kind: "contains" } },
            { data: { id: "e2", source: "svc1", target: "comp1", kind: "contains" } },
            { data: { id: "e3", source: "comp1", target: "unit1", kind: "contains" } },
            { data: { id: "e4", source: "comp1", target: "test1", kind: "contains" } },
          ],
        },
      };
    }

    it("all mode returns all nodes", () => {
      const graph = buildGraphWithTests();
      const result = filterGraph(graph, allKinds, allEdgeKinds, allSubKinds, allLanguages, "all");
      const ids = result.elements.nodes.map((n) => n.data.id);
      expect(ids).toContain("unit1");
      expect(ids).toContain("test1");
      expect(result.elements.nodes).toHaveLength(5);
    });

    it("code mode excludes nodes with test tag", () => {
      const graph = buildGraphWithTests();
      const result = filterGraph(graph, allKinds, allEdgeKinds, allSubKinds, allLanguages, "code");
      const ids = result.elements.nodes.map((n) => n.data.id);
      expect(ids).toContain("unit1");
      expect(ids).not.toContain("test1");
    });

    it("tests mode shows only nodes with test tag plus ancestors", () => {
      const graph = buildGraphWithTests();
      const result = filterGraph(graph, allKinds, allEdgeKinds, allSubKinds, allLanguages, "tests");
      const ids = result.elements.nodes.map((n) => n.data.id);
      // test1 passes directly
      expect(ids).toContain("test1");
      // ancestors are preserved
      expect(ids).toContain("comp1");
      expect(ids).toContain("svc1");
      expect(ids).toContain("sys");
      // unit1 does not have the test tag
      expect(ids).not.toContain("unit1");
    });
  });
});
