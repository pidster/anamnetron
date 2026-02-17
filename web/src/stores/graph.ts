import type { Snapshot, CytoscapeGraph, ConformanceReport, Version } from "../lib/types";

/** Reactive store for graph data and snapshot state. */
class GraphStore {
  snapshots = $state<Snapshot[]>([]);
  selectedVersion = $state<Version | null>(null);
  graph = $state<CytoscapeGraph | null>(null);
  conformanceReport = $state<ConformanceReport | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);

  /** Design snapshots only. */
  get designSnapshots(): Snapshot[] {
    return this.snapshots.filter((s) => s.kind === "design");
  }

  /** Analysis snapshots only. */
  get analysisSnapshots(): Snapshot[] {
    return this.snapshots.filter((s) => s.kind === "analysis");
  }

  /** Clear error state. */
  clearError() {
    this.error = null;
  }
}

export const graphStore = new GraphStore();
