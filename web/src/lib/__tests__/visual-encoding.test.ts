import { describe, it, expect } from "vitest";
import { KIND_COLORS, SUB_KIND_SHAPES, EDGE_STYLES } from "../visual-encoding";

describe("KIND_COLORS", () => {
  it("has an entry for every node kind", () => {
    for (const kind of ["system", "service", "component", "unit"]) {
      expect(KIND_COLORS[kind]).toBeDefined();
      expect(KIND_COLORS[kind]).toMatch(/^--kind-/);
    }
  });
});

describe("SUB_KIND_SHAPES", () => {
  it("maps trait to diamond", () => {
    expect(SUB_KIND_SHAPES["trait"]).toBe("diamond");
  });

  it("maps struct to hexagon", () => {
    expect(SUB_KIND_SHAPES["struct"]).toBe("hexagon");
  });

  it("maps function to ellipse", () => {
    expect(SUB_KIND_SHAPES["function"]).toBe("ellipse");
  });

  it("returns undefined for unknown sub_kind", () => {
    expect(SUB_KIND_SHAPES["unknown_type"]).toBeUndefined();
  });
});

describe("EDGE_STYLES", () => {
  it("covers all non-contains edge kinds", () => {
    for (const kind of ["depends", "calls", "implements", "extends", "exports", "data_flow"]) {
      expect(EDGE_STYLES[kind]).toBeDefined();
      expect(EDGE_STYLES[kind].cssVar).toBeTruthy();
      expect(EDGE_STYLES[kind].lineStyle).toBeTruthy();
      expect(EDGE_STYLES[kind].arrowShape).toBeTruthy();
    }
  });
});
