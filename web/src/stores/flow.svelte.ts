import type { RootAnalysis } from "../lib/api";
export type { RootEntry, RootAnalysis } from "../lib/api";

const ALL_EDGE_KINDS = ["depends", "calls", "implements", "extends", "data_flow", "exports"];

/** Reactive store for flow view state. */
class FlowStore {
  roots = $state<RootAnalysis | null>(null);
  expandedNodes = $state<Set<string>>(new Set());
  activeEdgeKinds = $state<Set<string>>(new Set(ALL_EDGE_KINDS));
  animationEnabled = $state(true);
  loading = $state(false);
  error = $state<string | null>(null);

  /** Toggle an edge kind on/off. */
  toggleEdgeKind(kind: string) {
    const next = new Set(this.activeEdgeKinds);
    if (next.has(kind)) {
      next.delete(kind);
    } else {
      next.add(kind);
    }
    this.activeEdgeKinds = next;
  }

  /** Toggle a node's expanded state (progressive disclosure). */
  toggleNode(nodeId: string) {
    const next = new Set(this.expandedNodes);
    if (next.has(nodeId)) {
      next.delete(nodeId);
    } else {
      next.add(nodeId);
    }
    this.expandedNodes = next;
  }

  /** Set root analysis data from API. */
  setRoots(roots: RootAnalysis) {
    this.roots = roots;
  }

  /** Reset all flow state. */
  reset() {
    this.roots = null;
    this.expandedNodes = new Set();
    this.activeEdgeKinds = new Set(ALL_EDGE_KINDS);
    this.animationEnabled = true;
    this.loading = false;
    this.error = null;
  }
}

export const flowStore = new FlowStore();
