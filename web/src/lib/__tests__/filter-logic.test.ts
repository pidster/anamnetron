import { describe, it, expect } from "vitest";
import {
  toggleInSet,
  extractSubKinds,
  extractLanguages,
  hasActiveFilters,
  formatLabel,
} from "../filter-logic";
import type { FilterState } from "../filter-logic";
import type { CyNodeData } from "../types";

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
