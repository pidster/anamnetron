import { describe, it, expect } from "vitest";
import { hierarchy, type HierarchyPointNode } from "d3-hierarchy";
import type { TreeNode } from "../hierarchy";
import {
  buildAncestorPaths,
  findPathThroughLCA,
  computeBundledEdges,
  computeArcEdges,
  createRadialCluster,
} from "../edge-bundling";

/** Helper to build a TreeNode hierarchy and lay it out radially. */
function makeTree(
  tree: TreeNode,
  radius = 200,
): HierarchyPointNode<TreeNode> {
  const root = hierarchy(tree) as HierarchyPointNode<TreeNode>;
  return createRadialCluster(root, radius);
}

function makeEdge(
  id: string,
  source: string,
  target: string,
  kind = "depends",
  _count?: number,
) {
  return { data: { id, source, target, kind, _count } };
}

const simpleTree: TreeNode = {
  id: "root",
  label: "Root",
  kind: "system",
  sub_kind: "",
  canonical_path: "/",
  children: [
    {
      id: "svc-a",
      label: "Service A",
      kind: "service",
      sub_kind: "",
      canonical_path: "/svc-a",
      children: [
        {
          id: "a1",
          label: "A1",
          kind: "unit",
          sub_kind: "",
          canonical_path: "/svc-a/a1",
          children: [],
        },
        {
          id: "a2",
          label: "A2",
          kind: "unit",
          sub_kind: "",
          canonical_path: "/svc-a/a2",
          children: [],
        },
      ],
    },
    {
      id: "svc-b",
      label: "Service B",
      kind: "service",
      sub_kind: "",
      canonical_path: "/svc-b",
      children: [
        {
          id: "b1",
          label: "B1",
          kind: "unit",
          sub_kind: "",
          canonical_path: "/svc-b/b1",
          children: [],
        },
      ],
    },
  ],
};

/** A flat tree: root with only leaf children (no intermediate hierarchy). */
const flatTree: TreeNode = {
  id: "root",
  label: "Root",
  kind: "system",
  sub_kind: "",
  canonical_path: "/",
  children: [
    {
      id: "mod-a",
      label: "Module A",
      kind: "component",
      sub_kind: "",
      canonical_path: "/mod-a",
      children: [],
    },
    {
      id: "mod-b",
      label: "Module B",
      kind: "component",
      sub_kind: "",
      canonical_path: "/mod-b",
      children: [],
    },
    {
      id: "mod-c",
      label: "Module C",
      kind: "component",
      sub_kind: "",
      canonical_path: "/mod-c",
      children: [],
    },
  ],
};

describe("buildAncestorPaths", () => {
  it("maps each node to its root-to-leaf path", () => {
    const root = makeTree(simpleTree);
    const paths = buildAncestorPaths(root);

    expect(paths.get("root")).toEqual(["root"]);
    expect(paths.get("svc-a")).toEqual(["root", "svc-a"]);
    expect(paths.get("a1")).toEqual(["root", "svc-a", "a1"]);
    expect(paths.get("b1")).toEqual(["root", "svc-b", "b1"]);
  });

  it("returns a path for every node in the tree", () => {
    const root = makeTree(simpleTree);
    const paths = buildAncestorPaths(root);
    // 6 nodes total: root, svc-a, a1, a2, svc-b, b1
    expect(paths.size).toBe(6);
  });
});

describe("findPathThroughLCA", () => {
  it("finds path between siblings (LCA is parent)", () => {
    const sourcePath = ["root", "svc-a", "a1"];
    const targetPath = ["root", "svc-a", "a2"];
    const result = findPathThroughLCA(sourcePath, targetPath);
    // a1 -> svc-a -> a2
    expect(result).toEqual(["a1", "svc-a", "a2"]);
  });

  it("finds path between cousins (LCA is grandparent)", () => {
    const sourcePath = ["root", "svc-a", "a1"];
    const targetPath = ["root", "svc-b", "b1"];
    const result = findPathThroughLCA(sourcePath, targetPath);
    // a1 -> svc-a -> root -> svc-b -> b1
    expect(result).toEqual(["a1", "svc-a", "root", "svc-b", "b1"]);
  });

  it("handles parent-child relationship", () => {
    const sourcePath = ["root", "svc-a"];
    const targetPath = ["root", "svc-a", "a1"];
    const result = findPathThroughLCA(sourcePath, targetPath);
    // svc-a -> a1
    expect(result).toEqual(["svc-a", "a1"]);
  });

  it("returns direct path for root to leaf", () => {
    const sourcePath = ["root"];
    const targetPath = ["root", "svc-a", "a1"];
    const result = findPathThroughLCA(sourcePath, targetPath);
    // root -> svc-a -> a1
    expect(result).toEqual(["root", "svc-a", "a1"]);
  });
});

describe("computeBundledEdges", () => {
  it("computes bundled edges for cross-subtree dependencies", () => {
    const root = makeTree(simpleTree);
    const edges = [makeEdge("e1", "a1", "b1", "depends")];
    const bundled = computeBundledEdges(root, edges);

    expect(bundled).toHaveLength(1);
    expect(bundled[0].sourceId).toBe("a1");
    expect(bundled[0].targetId).toBe("b1");
    expect(bundled[0].kind).toBe("depends");
    // Should have control points going through the hierarchy
    expect(bundled[0].points.length).toBeGreaterThanOrEqual(2);
  });

  it("skips contains edges", () => {
    const root = makeTree(simpleTree);
    const edges = [makeEdge("c1", "svc-a", "a1", "contains")];
    const bundled = computeBundledEdges(root, edges);
    expect(bundled).toHaveLength(0);
  });

  it("skips edges with dangling endpoints", () => {
    const root = makeTree(simpleTree);
    const edges = [makeEdge("e1", "a1", "nonexistent", "depends")];
    const bundled = computeBundledEdges(root, edges);
    expect(bundled).toHaveLength(0);
  });

  it("handles multiple edges", () => {
    const root = makeTree(simpleTree);
    const edges = [
      makeEdge("e1", "a1", "b1", "depends"),
      makeEdge("e2", "a2", "b1", "calls"),
      makeEdge("e3", "b1", "a1", "depends"),
    ];
    const bundled = computeBundledEdges(root, edges);
    expect(bundled).toHaveLength(3);
  });

  it("returns empty array for empty edges", () => {
    const root = makeTree(simpleTree);
    const bundled = computeBundledEdges(root, []);
    expect(bundled).toHaveLength(0);
  });

  it("preserves edge kind", () => {
    const root = makeTree(simpleTree);
    const edges = [
      makeEdge("e1", "a1", "b1", "calls"),
      makeEdge("e2", "a2", "b1", "implements"),
    ];
    const bundled = computeBundledEdges(root, edges);
    expect(bundled[0].kind).toBe("calls");
    expect(bundled[1].kind).toBe("implements");
  });

  it("each bundled edge has valid radial coordinates", () => {
    const root = makeTree(simpleTree);
    const edges = [makeEdge("e1", "a1", "b1", "depends")];
    const bundled = computeBundledEdges(root, edges);

    for (const point of bundled[0].points) {
      const [angle, radius] = point;
      // Angle should be in radians (0 to 2*PI)
      expect(angle).toBeGreaterThanOrEqual(0);
      expect(angle).toBeLessThanOrEqual(2 * Math.PI);
      // Radius should be non-negative
      expect(radius).toBeGreaterThanOrEqual(0);
    }
  });

  it("propagates _count to count field", () => {
    const root = makeTree(simpleTree);
    const edges = [makeEdge("e1", "a1", "b1", "depends", 7)];
    const bundled = computeBundledEdges(root, edges);

    expect(bundled).toHaveLength(1);
    expect(bundled[0].count).toBe(7);
  });

  it("defaults count to 1 when _count is undefined", () => {
    const root = makeTree(simpleTree);
    const edges = [makeEdge("e1", "a1", "b1", "depends")];
    const bundled = computeBundledEdges(root, edges);

    expect(bundled).toHaveLength(1);
    expect(bundled[0].count).toBe(1);
  });
});

describe("computeArcEdges", () => {
  it("produces 3-point arcs for a flat tree", () => {
    const root = makeTree(flatTree);
    const edges = [makeEdge("e1", "mod-a", "mod-b", "depends")];
    const arcs = computeArcEdges(root, edges);

    expect(arcs).toHaveLength(1);
    expect(arcs[0].sourceId).toBe("mod-a");
    expect(arcs[0].targetId).toBe("mod-b");
    expect(arcs[0].points).toHaveLength(3);
  });

  it("skips contains edges", () => {
    const root = makeTree(flatTree);
    const edges = [makeEdge("c1", "root", "mod-a", "contains")];
    const arcs = computeArcEdges(root, edges);
    expect(arcs).toHaveLength(0);
  });

  it("skips edges with dangling endpoints", () => {
    const root = makeTree(flatTree);
    const edges = [makeEdge("e1", "mod-a", "nonexistent", "depends")];
    const arcs = computeArcEdges(root, edges);
    expect(arcs).toHaveLength(0);
  });

  it("carries count from _count", () => {
    const root = makeTree(flatTree);
    const edges = [makeEdge("e1", "mod-a", "mod-b", "depends", 12)];
    const arcs = computeArcEdges(root, edges);

    expect(arcs).toHaveLength(1);
    expect(arcs[0].count).toBe(12);
  });

  it("defaults count to 1 when _count is undefined", () => {
    const root = makeTree(flatTree);
    const edges = [makeEdge("e1", "mod-a", "mod-b", "depends")];
    const arcs = computeArcEdges(root, edges);

    expect(arcs).toHaveLength(1);
    expect(arcs[0].count).toBe(1);
  });

  it("control point radius is pulled inward", () => {
    const root = makeTree(flatTree, 200);
    const edges = [makeEdge("e1", "mod-a", "mod-b", "depends")];
    const arcs = computeArcEdges(root, edges);

    const [, controlPoint] = [arcs[0].points[0], arcs[0].points[1]];
    // Control point radius should be much smaller than the leaf radius (200)
    expect(controlPoint[1]).toBeLessThan(200 * 0.5);
  });

  it("handles multiple edges", () => {
    const root = makeTree(flatTree);
    const edges = [
      makeEdge("e1", "mod-a", "mod-b", "depends"),
      makeEdge("e2", "mod-b", "mod-c", "calls"),
      makeEdge("e3", "mod-c", "mod-a", "depends"),
    ];
    const arcs = computeArcEdges(root, edges);
    expect(arcs).toHaveLength(3);
    // Each should have 3 points
    for (const arc of arcs) {
      expect(arc.points).toHaveLength(3);
    }
  });
});

describe("flat tree detection", () => {
  it("flat tree has hierarchy height of 1", () => {
    const root = makeTree(flatTree);
    expect(root.height).toBe(1);
  });

  it("deeper tree has hierarchy height > 1", () => {
    const root = makeTree(simpleTree);
    expect(root.height).toBeGreaterThan(1);
  });
});

describe("createRadialCluster", () => {
  it("assigns positions to all nodes", () => {
    const root = makeTree(simpleTree);
    let count = 0;
    root.each((node) => {
      expect(typeof node.x).toBe("number");
      expect(typeof node.y).toBe("number");
      count++;
    });
    expect(count).toBe(6);
  });

  it("places leaves on the outer ring", () => {
    const root = makeTree(simpleTree, 200);
    const leaves = root.leaves();
    for (const leaf of leaves) {
      // Leaves should be at the specified radius
      expect(leaf.y).toBe(200);
    }
  });

  it("places root at minimum radius (not center)", () => {
    const root = makeTree(simpleTree);
    // Root is remapped to minRadiusFraction * innerRadius (default 0.4 * 200)
    expect(root.y).toBeCloseTo(200 * 0.4, 1);
  });
});
