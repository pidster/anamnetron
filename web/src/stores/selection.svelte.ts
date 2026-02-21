import type { ApiNode, ApiEdge } from "../lib/types";

/** Reactive store for node selection and detail panel state. */
class SelectionStore {
  selectedNodeId = $state<string | null>(null);
  selectedNode = $state<ApiNode | null>(null);
  children = $state<ApiNode[]>([]);
  ancestors = $state<ApiNode[]>([]);
  dependencies = $state<ApiEdge[]>([]);
  dependents = $state<ApiEdge[]>([]);
  panelOpen = $state(false);
  loading = $state(false);

  /** Clear selection and close panel. */
  clear() {
    this.selectedNodeId = null;
    this.selectedNode = null;
    this.children = [];
    this.ancestors = [];
    this.dependencies = [];
    this.dependents = [];
    this.panelOpen = false;
  }
}

export const selectionStore = new SelectionStore();
