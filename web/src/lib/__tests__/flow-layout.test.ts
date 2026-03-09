import { describe, it, expect } from "vitest";
import { buildFlowElements, computeClientRoots } from "../flow-layout";
import type { CytoscapeGraph, CyNodeData } from "../types";
import type { RootAnalysis } from "../api";

/** Create a graph node with sensible defaults. */
function makeNode(overrides: Partial<CyNodeData> = {}): { data: CyNodeData } {
  return {
    data: {
      id: overrides.id ?? "n1",
      label: overrides.label ?? "Node 1",
      kind: overrides.kind ?? "component",
      sub_kind: overrides.sub_kind ?? "module",
      canonical_path: overrides.canonical_path ?? `/${overrides.id ?? "n1"}`,
      ...(overrides.parent ? { parent: overrides.parent } : {}),
      ...(overrides.language ? { language: overrides.language } : {}),
      ...(overrides.metadata ? { metadata: overrides.metadata } : {}),
    },
  };
}

/** Create an edge. */
function makeEdge(id: string, source: string, target: string, kind: string) {
  return { data: { id, source, target, kind } };
}

/** Build a CytoscapeGraph from arrays of nodes and edges. */
function makeGraph(
  nodes: Array<{ data: CyNodeData }>,
  edges: Array<{ data: { id: string; source: string; target: string; kind: string } }> = [],
): CytoscapeGraph {
  return { elements: { nodes, edges } };
}

/** Create an empty RootAnalysis. */
function emptyRoots(): RootAnalysis {
  return {
    call_tree_roots: [],
    dependency_sources: [],
    dependency_sinks: [],
    containment_roots: [],
    leaf_sinks: [],
  };
}

/** Generate N child nodes under a parent for large-graph tests. */
function generateChildren(parentId: string, count: number, kind: string = "unit"): Array<{ data: CyNodeData }> {
  const nodes: Array<{ data: CyNodeData }> = [];
  for (let i = 0; i < count; i++) {
    nodes.push(makeNode({ id: `${parentId}_child_${i}`, label: `Child ${i}`, kind, sub_kind: "function", parent: parentId }));
  }
  return nodes;
}

describe("buildFlowElements", () => {
  it("returns empty nodes and edges for an empty graph", () => {
    const graph = makeGraph([], []);
    const result = buildFlowElements(graph, null, new Set());
    expect(result.nodes).toHaveLength(0);
    expect(result.edges).toHaveLength(0);
  });

  it("passes through all nodes when graph is below MAX_VISIBLE_NODES", () => {
    const nodes = [
      makeNode({ id: "sys", kind: "system", sub_kind: "workspace" }),
      makeNode({ id: "svc", kind: "service", sub_kind: "crate", parent: "sys" }),
      makeNode({ id: "comp", kind: "component", sub_kind: "module", parent: "svc" }),
      makeNode({ id: "unit1", kind: "unit", sub_kind: "function", parent: "comp" }),
    ];
    const edges = [
      makeEdge("e1", "unit1", "comp", "calls"),
    ];
    const graph = makeGraph(nodes, edges);
    const result = buildFlowElements(graph, null, new Set());

    expect(result.nodes).toHaveLength(4);
    expect(result.edges).toHaveLength(1);
  });

  it("collapses component and unit nodes when graph exceeds MAX_VISIBLE_NODES", () => {
    // Build a graph with >200 nodes: 1 system + 1 service + 1 component + 210 unit children
    const sys = makeNode({ id: "sys", kind: "system", sub_kind: "workspace" });
    const svc = makeNode({ id: "svc", kind: "service", sub_kind: "crate", parent: "sys" });
    const comp = makeNode({ id: "comp", kind: "component", sub_kind: "module", parent: "svc" });
    const children = generateChildren("comp", 210);
    const graph = makeGraph([sys, svc, comp, ...children], []);
    const result = buildFlowElements(graph, null, new Set());

    // comp should be collapsed, its children hidden
    const compNode = result.nodes.find((n) => n.data.id === "comp");
    expect(compNode).toBeDefined();
    expect(compNode!.data.collapsed).toBe(true);
    expect(compNode!.data.childCount).toBe(210);

    // Hidden children should not appear
    const childIds = result.nodes.filter((n) => n.data.id.startsWith("comp_child_"));
    expect(childIds).toHaveLength(0);
  });

  it("applies aggressive collapsing when first pass is not enough", () => {
    // Create a graph where component/unit collapsing isn't enough.
    // Use "service" nodes with >3 children each to trigger aggressive pass.
    const sys = makeNode({ id: "sys", kind: "system", sub_kind: "workspace" });
    // Create 50 service nodes, each with 5 children (service kind won't be collapsed in first pass)
    const serviceNodes: Array<{ data: CyNodeData }> = [];
    const childNodes: Array<{ data: CyNodeData }> = [];
    for (let i = 0; i < 50; i++) {
      serviceNodes.push(makeNode({ id: `svc_${i}`, kind: "service", sub_kind: "crate", parent: "sys" }));
      for (let j = 0; j < 5; j++) {
        childNodes.push(makeNode({ id: `svc_${i}_child_${j}`, kind: "service", sub_kind: "module", parent: `svc_${i}` }));
      }
    }
    const graph = makeGraph([sys, ...serviceNodes, ...childNodes], []);
    // Total: 1 + 50 + 250 = 301 nodes, exceeds 200
    const result = buildFlowElements(graph, null, new Set());

    // Aggressive collapsing should collapse service nodes with >3 children
    const collapsedServices = result.nodes.filter((n) => n.data.collapsed === true);
    expect(collapsedServices.length).toBeGreaterThan(0);

    // Total visible nodes should be reduced
    expect(result.nodes.length).toBeLessThan(301);
  });

  it("expands explicitly expanded nodes, making direct children visible", () => {
    const sys = makeNode({ id: "sys", kind: "system", sub_kind: "workspace" });
    const svc = makeNode({ id: "svc", kind: "service", sub_kind: "crate", parent: "sys" });
    const comp = makeNode({ id: "comp", kind: "component", sub_kind: "module", parent: "svc" });
    const children = generateChildren("comp", 210);
    const graph = makeGraph([sys, svc, comp, ...children], []);

    // Expand comp — its direct children should become visible
    const result = buildFlowElements(graph, null, new Set(["comp"]));

    const compNode = result.nodes.find((n) => n.data.id === "comp");
    expect(compNode).toBeDefined();
    // comp should no longer be collapsed since it's expanded
    expect(compNode!.data.collapsed).toBeUndefined();

    // Direct children should be visible
    const visibleChildren = result.nodes.filter((n) => n.data.id.startsWith("comp_child_"));
    expect(visibleChildren.length).toBe(210);
  });

  it("adds 'collapsed' CSS class and childCount data to collapsed nodes", () => {
    const sys = makeNode({ id: "sys", kind: "system", sub_kind: "workspace" });
    const comp = makeNode({ id: "comp", kind: "component", sub_kind: "module", parent: "sys" });
    const children = generateChildren("comp", 205);
    const graph = makeGraph([sys, comp, ...children], []);
    const result = buildFlowElements(graph, null, new Set());

    const compNode = result.nodes.find((n) => n.data.id === "comp");
    expect(compNode).toBeDefined();
    expect(compNode!.classes).toContain("collapsed");
    expect(compNode!.data.collapsed).toBe(true);
    expect(compNode!.data.childCount).toBe(205);
  });

  it("adds 'root' CSS class and rootCategory data to root nodes", () => {
    const nodes = [
      makeNode({ id: "r1", kind: "component", sub_kind: "module" }),
      makeNode({ id: "r2", kind: "service", sub_kind: "crate" }),
    ];
    const roots: RootAnalysis = {
      ...emptyRoots(),
      call_tree_roots: [{ node_id: "r1", canonical_path: "/r1", name: "Root 1" }],
      containment_roots: [{ node_id: "r2", canonical_path: "/r2", name: "Root 2" }],
    };
    const graph = makeGraph(nodes, []);
    const result = buildFlowElements(graph, roots, new Set());

    const r1Node = result.nodes.find((n) => n.data.id === "r1");
    expect(r1Node).toBeDefined();
    expect(r1Node!.classes).toContain("root");
    expect(r1Node!.classes).toContain("call-tree-root");
    expect(r1Node!.data.rootCategory).toBe("call_tree_root");

    const r2Node = result.nodes.find((n) => n.data.id === "r2");
    expect(r2Node).toBeDefined();
    expect(r2Node!.classes).toContain("root");
    expect(r2Node!.classes).toContain("containment-root");
    expect(r2Node!.data.rootCategory).toBe("containment_root");
  });

  it("remaps edges involving hidden nodes to their collapsed parent", () => {
    const sys = makeNode({ id: "sys", kind: "system", sub_kind: "workspace" });
    const comp = makeNode({ id: "comp", kind: "component", sub_kind: "module", parent: "sys" });
    const children = generateChildren("comp", 205);
    const otherNode = makeNode({ id: "other", kind: "service", sub_kind: "crate", parent: "sys" });

    // Edge from hidden child to a visible node
    const edges = [
      makeEdge("e1", "comp_child_0", "other", "calls"),
    ];
    const graph = makeGraph([sys, comp, ...children, otherNode], edges);
    const result = buildFlowElements(graph, null, new Set());

    // Edge should be remapped: source becomes "comp" (collapsed parent)
    const remappedEdge = result.edges.find((e) => e.data.target === "other");
    expect(remappedEdge).toBeDefined();
    expect(remappedEdge!.data.source).toBe("comp");
  });

  it("removes self-loop edges created by collapsing", () => {
    const sys = makeNode({ id: "sys", kind: "system", sub_kind: "workspace" });
    const comp = makeNode({ id: "comp", kind: "component", sub_kind: "module", parent: "sys" });
    const children = generateChildren("comp", 205);

    // Edge between two children of the same collapsed parent — becomes self-loop
    const edges = [
      makeEdge("e1", "comp_child_0", "comp_child_1", "calls"),
    ];
    const graph = makeGraph([sys, comp, ...children], edges);
    const result = buildFlowElements(graph, null, new Set());

    // Self-loop should be removed
    expect(result.edges).toHaveLength(0);
  });

  it("aggregates edges with same source, target, and kind", () => {
    const nodes = [
      makeNode({ id: "a", kind: "component", sub_kind: "module" }),
      makeNode({ id: "b", kind: "component", sub_kind: "module" }),
    ];
    const edges = [
      makeEdge("e1", "a", "b", "calls"),
      makeEdge("e2", "a", "b", "calls"),
      makeEdge("e3", "a", "b", "calls"),
    ];
    const graph = makeGraph(nodes, edges);
    const result = buildFlowElements(graph, null, new Set());

    // Should be aggregated into a single edge
    expect(result.edges).toHaveLength(1);
    expect(result.edges[0].data.count).toBe(3);
    expect(result.edges[0].data.kind).toBe("calls");
  });

  it("does not aggregate edges with different kinds", () => {
    const nodes = [
      makeNode({ id: "a", kind: "component", sub_kind: "module" }),
      makeNode({ id: "b", kind: "component", sub_kind: "module" }),
    ];
    const edges = [
      makeEdge("e1", "a", "b", "calls"),
      makeEdge("e2", "a", "b", "depends"),
    ];
    const graph = makeGraph(nodes, edges);
    const result = buildFlowElements(graph, null, new Set());

    expect(result.edges).toHaveLength(2);
  });

  it("filters out 'contains' edges", () => {
    const nodes = [
      makeNode({ id: "parent", kind: "system", sub_kind: "workspace" }),
      makeNode({ id: "child", kind: "service", sub_kind: "crate", parent: "parent" }),
    ];
    const edges = [
      makeEdge("e1", "parent", "child", "contains"),
      makeEdge("e2", "child", "parent", "calls"),
    ];
    const graph = makeGraph(nodes, edges);
    const result = buildFlowElements(graph, null, new Set());

    // "contains" edge should be filtered, "calls" should remain
    expect(result.edges).toHaveLength(1);
    expect(result.edges[0].data.kind).toBe("calls");
  });

  it("sets parent field only when parent exists in nodeMap and is not hidden", () => {
    const nodes = [
      makeNode({ id: "sys", kind: "system", sub_kind: "workspace" }),
      makeNode({ id: "comp", kind: "component", sub_kind: "module", parent: "sys" }),
      // A node whose parent is not in the graph
      makeNode({ id: "orphan", kind: "unit", sub_kind: "function", parent: "missing_parent" }),
    ];
    const graph = makeGraph(nodes, []);
    const result = buildFlowElements(graph, null, new Set());

    const compNode = result.nodes.find((n) => n.data.id === "comp");
    expect(compNode!.data.parent).toBe("sys");

    const orphanNode = result.nodes.find((n) => n.data.id === "orphan");
    expect(orphanNode!.data.parent).toBeUndefined();
  });

  it("applies node kind CSS classes", () => {
    const nodes = [
      makeNode({ id: "sys", kind: "system", sub_kind: "workspace" }),
      makeNode({ id: "svc", kind: "service", sub_kind: "crate" }),
      makeNode({ id: "comp", kind: "component", sub_kind: "module" }),
      makeNode({ id: "unit", kind: "unit", sub_kind: "function" }),
    ];
    const graph = makeGraph(nodes, []);
    const result = buildFlowElements(graph, null, new Set());

    expect(result.nodes.find((n) => n.data.id === "sys")!.classes).toContain("kind-system");
    expect(result.nodes.find((n) => n.data.id === "svc")!.classes).toContain("kind-service");
    expect(result.nodes.find((n) => n.data.id === "comp")!.classes).toContain("kind-component");
    expect(result.nodes.find((n) => n.data.id === "unit")!.classes).toContain("kind-unit");
  });

  it("handles a single node graph", () => {
    const graph = makeGraph([makeNode({ id: "solo", kind: "system", sub_kind: "workspace" })], []);
    const result = buildFlowElements(graph, null, new Set());

    expect(result.nodes).toHaveLength(1);
    expect(result.edges).toHaveLength(0);
    expect(result.nodes[0].data.id).toBe("solo");
  });

  it("handles graph with cycles in call edges", () => {
    const nodes = [
      makeNode({ id: "a", kind: "component", sub_kind: "module" }),
      makeNode({ id: "b", kind: "component", sub_kind: "module" }),
    ];
    const edges = [
      makeEdge("e1", "a", "b", "calls"),
      makeEdge("e2", "b", "a", "calls"),
    ];
    const graph = makeGraph(nodes, edges);
    const result = buildFlowElements(graph, null, new Set());

    expect(result.nodes).toHaveLength(2);
    expect(result.edges).toHaveLength(2);
  });

  it("handles null roots gracefully", () => {
    const graph = makeGraph([makeNode({ id: "a", kind: "component", sub_kind: "module" })], []);
    const result = buildFlowElements(graph, null, new Set());

    expect(result.nodes).toHaveLength(1);
    expect(result.nodes[0].data.rootCategory).toBeUndefined();
    expect(result.nodes[0].classes).toBe("kind-component");
  });

  it("preserves canonical_path and language on output nodes", () => {
    const graph = makeGraph(
      [makeNode({ id: "a", kind: "component", sub_kind: "module", language: "rust", canonical_path: "/src/main.rs" })],
      [],
    );
    const result = buildFlowElements(graph, null, new Set());

    expect(result.nodes[0].data.canonical_path).toBe("/src/main.rs");
    expect(result.nodes[0].data.language).toBe("rust");
  });

  it("skips first-pass collapsing when expandedNodes is non-empty but aggressive pass still applies", () => {
    // The first pass (component/unit collapsing) requires expandedNodes.size === 0,
    // but the aggressive second pass (>3 children threshold) runs independently.
    const sys = makeNode({ id: "sys", kind: "system", sub_kind: "workspace" });
    const comp = makeNode({ id: "comp", kind: "component", sub_kind: "module", parent: "sys" });
    const children = generateChildren("comp", 210);
    const graph = makeGraph([sys, comp, ...children], []);

    // Pass a non-empty expandedNodes set — first pass is skipped but aggressive pass still applies
    const result = buildFlowElements(graph, null, new Set(["some_other_node"]));

    // comp has >3 children and isn't in expandedNodes, so aggressive pass collapses it
    const compNode = result.nodes.find((n) => n.data.id === "comp");
    expect(compNode!.data.collapsed).toBe(true);
    // Visible nodes: sys + comp (collapsed) = 2
    expect(result.nodes).toHaveLength(2);
  });

  it("aggressive pass collapses nodes to bring count under MAX_VISIBLE_NODES", () => {
    // Create a graph >200 nodes where parents have small child counts.
    // The aggressive pass collapses any node with children to reduce count.
    const topNodes: Array<{ data: CyNodeData }> = [];
    // 70 independent parent nodes, each with exactly 3 children = 70 + 210 = 280 nodes
    for (let i = 0; i < 70; i++) {
      topNodes.push(makeNode({ id: `parent_${i}`, kind: "service", sub_kind: "crate" }));
      for (let j = 0; j < 3; j++) {
        topNodes.push(makeNode({ id: `parent_${i}_child_${j}`, kind: "unit", sub_kind: "function", parent: `parent_${i}` }));
      }
    }
    const graph = makeGraph(topNodes, []);
    // Total: 280 nodes, exceeds 200. Medium pass collapses unit containers (none here
    // since units are leaves). Aggressive pass collapses service parents.
    const result = buildFlowElements(graph, null, new Set(["some_other_node"]));

    const collapsedNodes = result.nodes.filter((n) => n.data.collapsed === true);
    expect(collapsedNodes.length).toBeGreaterThan(0);
    // Visible nodes should be reduced below 280
    expect(result.nodes.length).toBeLessThan(280);
  });

  it("deeply nested hidden nodes are collected as descendants", () => {
    // comp -> child -> grandchild (3 levels)
    const sys = makeNode({ id: "sys", kind: "system", sub_kind: "workspace" });
    const comp = makeNode({ id: "comp", kind: "component", sub_kind: "module", parent: "sys" });
    const children: Array<{ data: CyNodeData }> = [];
    // Create 100 children, each with 2 grandchildren = 300 descendant nodes total
    for (let i = 0; i < 100; i++) {
      children.push(makeNode({ id: `child_${i}`, kind: "unit", sub_kind: "function", parent: "comp" }));
      children.push(makeNode({ id: `grandchild_${i}_0`, kind: "unit", sub_kind: "function", parent: `child_${i}` }));
      children.push(makeNode({ id: `grandchild_${i}_1`, kind: "unit", sub_kind: "function", parent: `child_${i}` }));
    }
    const graph = makeGraph([sys, comp, ...children], []);
    const result = buildFlowElements(graph, null, new Set());

    // comp should be collapsed
    const compNode = result.nodes.find((n) => n.data.id === "comp");
    expect(compNode!.data.collapsed).toBe(true);
    // childCount should include all descendants (children + grandchildren)
    expect(compNode!.data.childCount).toBe(300);

    // No descendants should be visible
    const descendantNodes = result.nodes.filter((n) => n.data.id.startsWith("child_") || n.data.id.startsWith("grandchild_"));
    expect(descendantNodes).toHaveLength(0);
  });
});

describe("computeClientRoots", () => {
  it("returns empty RootAnalysis for an empty graph", () => {
    const graph = makeGraph([], []);
    const result = computeClientRoots(graph);

    expect(result.call_tree_roots).toHaveLength(0);
    expect(result.dependency_sources).toHaveLength(0);
    expect(result.dependency_sinks).toHaveLength(0);
    expect(result.containment_roots).toHaveLength(0);
    expect(result.leaf_sinks).toHaveLength(0);
  });

  it("identifies nodes with outgoing calls but no incoming calls as call_tree_roots", () => {
    const nodes = [
      makeNode({ id: "caller", label: "Caller", kind: "component", sub_kind: "module" }),
      makeNode({ id: "callee", label: "Callee", kind: "component", sub_kind: "module" }),
    ];
    const edges = [makeEdge("e1", "caller", "callee", "calls")];
    const graph = makeGraph(nodes, edges);
    const result = computeClientRoots(graph);

    expect(result.call_tree_roots).toHaveLength(1);
    expect(result.call_tree_roots[0].node_id).toBe("caller");
    expect(result.call_tree_roots[0].name).toBe("Caller");
    expect(result.call_tree_roots[0].canonical_path).toBe("/caller");
  });

  it("does not mark nodes with both incoming and outgoing calls as call_tree_roots", () => {
    const nodes = [
      makeNode({ id: "a", label: "A", kind: "component", sub_kind: "module" }),
      makeNode({ id: "b", label: "B", kind: "component", sub_kind: "module" }),
      makeNode({ id: "c", label: "C", kind: "component", sub_kind: "module" }),
    ];
    const edges = [
      makeEdge("e1", "a", "b", "calls"),
      makeEdge("e2", "b", "c", "calls"),
    ];
    const graph = makeGraph(nodes, edges);
    const result = computeClientRoots(graph);

    // Only "a" has outgoing calls with no incoming calls
    expect(result.call_tree_roots).toHaveLength(1);
    expect(result.call_tree_roots[0].node_id).toBe("a");

    // "b" has both incoming and outgoing — should NOT be a root
    const bRoot = result.call_tree_roots.find((r) => r.node_id === "b");
    expect(bRoot).toBeUndefined();
  });

  it("identifies system/service nodes without parents as containment_roots", () => {
    const nodes = [
      makeNode({ id: "sys", label: "My System", kind: "system", sub_kind: "workspace" }),
      makeNode({ id: "svc", label: "My Service", kind: "service", sub_kind: "crate" }),
      makeNode({ id: "nested_svc", label: "Nested", kind: "service", sub_kind: "crate", parent: "sys" }),
    ];
    const graph = makeGraph(nodes, []);
    const result = computeClientRoots(graph);

    // sys and svc have no parent — should be containment roots
    expect(result.containment_roots).toHaveLength(2);
    const rootIds = result.containment_roots.map((r) => r.node_id);
    expect(rootIds).toContain("sys");
    expect(rootIds).toContain("svc");
    // nested_svc has a parent — should NOT be a containment root
    expect(rootIds).not.toContain("nested_svc");
  });

  it("does not include component or unit nodes as containment_roots", () => {
    const nodes = [
      makeNode({ id: "comp", label: "Comp", kind: "component", sub_kind: "module" }),
      makeNode({ id: "unit", label: "Unit", kind: "unit", sub_kind: "function" }),
    ];
    const graph = makeGraph(nodes, []);
    const result = computeClientRoots(graph);

    expect(result.containment_roots).toHaveLength(0);
  });

  it("always returns empty dependency_sources, dependency_sinks, and leaf_sinks", () => {
    const nodes = [
      makeNode({ id: "a", label: "A", kind: "component", sub_kind: "module" }),
      makeNode({ id: "b", label: "B", kind: "component", sub_kind: "module" }),
    ];
    const edges = [
      makeEdge("e1", "a", "b", "depends"),
      makeEdge("e2", "a", "b", "calls"),
    ];
    const graph = makeGraph(nodes, edges);
    const result = computeClientRoots(graph);

    expect(result.dependency_sources).toHaveLength(0);
    expect(result.dependency_sinks).toHaveLength(0);
    expect(result.leaf_sinks).toHaveLength(0);
  });

  it("only counts 'calls' edges for call tree root determination, not other kinds", () => {
    const nodes = [
      makeNode({ id: "a", label: "A", kind: "component", sub_kind: "module" }),
      makeNode({ id: "b", label: "B", kind: "component", sub_kind: "module" }),
    ];
    // Only "depends" edges, no "calls"
    const edges = [makeEdge("e1", "a", "b", "depends")];
    const graph = makeGraph(nodes, edges);
    const result = computeClientRoots(graph);

    // No node has outgoing "calls" so no call_tree_roots
    expect(result.call_tree_roots).toHaveLength(0);
  });

  it("handles a single node with no edges", () => {
    const graph = makeGraph([makeNode({ id: "solo", label: "Solo", kind: "system", sub_kind: "workspace" })], []);
    const result = computeClientRoots(graph);

    // No calls, so no call_tree_roots
    expect(result.call_tree_roots).toHaveLength(0);
    // system node without parent is a containment root
    expect(result.containment_roots).toHaveLength(1);
    expect(result.containment_roots[0].node_id).toBe("solo");
  });

  it("handles cycles in call edges", () => {
    const nodes = [
      makeNode({ id: "a", label: "A", kind: "component", sub_kind: "module" }),
      makeNode({ id: "b", label: "B", kind: "component", sub_kind: "module" }),
    ];
    const edges = [
      makeEdge("e1", "a", "b", "calls"),
      makeEdge("e2", "b", "a", "calls"),
    ];
    const graph = makeGraph(nodes, edges);
    const result = computeClientRoots(graph);

    // Both have incoming and outgoing — neither should be a call tree root
    expect(result.call_tree_roots).toHaveLength(0);
  });
});
