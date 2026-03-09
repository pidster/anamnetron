import { describe, it, expect } from "vitest";
import { flowStore } from "../../stores/flow.svelte";
import type { RootAnalysis } from "../api";

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

const ALL_EDGE_KINDS = ["depends", "calls", "implements", "extends", "transforms", "data_flow", "exports"];

describe("FlowStore", () => {
  // Reset before each test to ensure isolation
  beforeEach(() => {
    flowStore.reset();
  });

  describe("initial state", () => {
    it("has all edge kinds active", () => {
      expect(flowStore.activeEdgeKinds).toEqual(new Set(ALL_EDGE_KINDS));
    });

    it("has animation enabled", () => {
      expect(flowStore.animationEnabled).toBe(true);
    });

    it("has no expanded nodes", () => {
      expect(flowStore.expandedNodes).toEqual(new Set());
    });

    it("has null roots", () => {
      expect(flowStore.roots).toBeNull();
    });

    it("is not loading", () => {
      expect(flowStore.loading).toBe(false);
    });

    it("has no error", () => {
      expect(flowStore.error).toBeNull();
    });
  });

  describe("toggleEdgeKind", () => {
    it("removes an edge kind when it is currently active", () => {
      flowStore.toggleEdgeKind("calls");
      expect(flowStore.activeEdgeKinds.has("calls")).toBe(false);
      // Other kinds should remain
      expect(flowStore.activeEdgeKinds.has("depends")).toBe(true);
    });

    it("adds an edge kind when it is currently inactive", () => {
      // First remove it
      flowStore.toggleEdgeKind("calls");
      expect(flowStore.activeEdgeKinds.has("calls")).toBe(false);

      // Then toggle it back
      flowStore.toggleEdgeKind("calls");
      expect(flowStore.activeEdgeKinds.has("calls")).toBe(true);
    });

    it("does not mutate the previous set reference", () => {
      const before = flowStore.activeEdgeKinds;
      flowStore.toggleEdgeKind("calls");
      // Should be a new set instance
      expect(flowStore.activeEdgeKinds).not.toBe(before);
    });
  });

  describe("toggleNode", () => {
    it("adds a node when it is not currently expanded", () => {
      flowStore.toggleNode("node-1");
      expect(flowStore.expandedNodes.has("node-1")).toBe(true);
    });

    it("removes a node when it is currently expanded", () => {
      flowStore.toggleNode("node-1");
      expect(flowStore.expandedNodes.has("node-1")).toBe(true);

      flowStore.toggleNode("node-1");
      expect(flowStore.expandedNodes.has("node-1")).toBe(false);
    });

    it("can expand multiple nodes independently", () => {
      flowStore.toggleNode("node-1");
      flowStore.toggleNode("node-2");
      expect(flowStore.expandedNodes.has("node-1")).toBe(true);
      expect(flowStore.expandedNodes.has("node-2")).toBe(true);
    });

    it("does not mutate the previous set reference", () => {
      const before = flowStore.expandedNodes;
      flowStore.toggleNode("node-1");
      expect(flowStore.expandedNodes).not.toBe(before);
    });
  });

  describe("setRoots", () => {
    it("updates roots state", () => {
      const roots: RootAnalysis = {
        ...emptyRoots(),
        call_tree_roots: [{ node_id: "r1", canonical_path: "/r1", name: "Root 1" }],
      };
      flowStore.setRoots(roots);
      expect(flowStore.roots).toStrictEqual(roots);
      expect(flowStore.roots!.call_tree_roots).toHaveLength(1);
      expect(flowStore.roots!.call_tree_roots[0].node_id).toBe("r1");
    });

    it("replaces previous roots", () => {
      const roots1: RootAnalysis = {
        ...emptyRoots(),
        call_tree_roots: [{ node_id: "r1", canonical_path: "/r1", name: "Root 1" }],
      };
      const roots2: RootAnalysis = {
        ...emptyRoots(),
        containment_roots: [{ node_id: "c1", canonical_path: "/c1", name: "Container 1" }],
      };
      flowStore.setRoots(roots1);
      flowStore.setRoots(roots2);
      expect(flowStore.roots).toStrictEqual(roots2);
      expect(flowStore.roots!.call_tree_roots).toHaveLength(0);
      expect(flowStore.roots!.containment_roots).toHaveLength(1);
    });
  });

  describe("reset", () => {
    it("clears all state back to defaults", () => {
      // Modify all state fields
      flowStore.toggleEdgeKind("calls");
      flowStore.toggleNode("node-1");
      flowStore.setRoots(emptyRoots());
      flowStore.loading = true;
      flowStore.error = "Something went wrong";
      flowStore.animationEnabled = false;

      // Reset
      flowStore.reset();

      // Verify all fields are back to defaults
      expect(flowStore.roots).toBeNull();
      expect(flowStore.expandedNodes).toEqual(new Set());
      expect(flowStore.activeEdgeKinds).toEqual(new Set(ALL_EDGE_KINDS));
      expect(flowStore.animationEnabled).toBe(true);
      expect(flowStore.loading).toBe(false);
      expect(flowStore.error).toBeNull();
    });

    it("can be called multiple times safely", () => {
      flowStore.reset();
      flowStore.reset();
      expect(flowStore.activeEdgeKinds).toEqual(new Set(ALL_EDGE_KINDS));
    });
  });
});
