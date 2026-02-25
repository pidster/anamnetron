import { describe, it, expect } from "vitest";
import { hierarchy, type HierarchyPointNode } from "d3-hierarchy";
import type { TreeNode } from "../hierarchy";
import {
  buildAncestorPaths,
  findPathThroughLCA,
  computeBundledEdges,
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
) {
  return { data: { id, source, target, kind } };
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

  it("places root at center", () => {
    const root = makeTree(simpleTree);
    expect(root.y).toBe(0);
  });
});
