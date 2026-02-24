import type { ApiNode, ApiEdge } from "../lib/types";

/** Reactive store for node selection and detail panel state. */
class SelectionStore {
  selectedNodeId = $state<string | null>(null);
  selectedNodeIds = $state<Set<string>>(new Set());
  selectedNode = $state<ApiNode | null>(null);
  children = $state<ApiNode[]>([]);
  ancestors = $state<ApiNode[]>([]);
  dependencies = $state<ApiEdge[]>([]);
  dependents = $state<ApiEdge[]>([]);
  panelOpen = $state(false);
  loading = $state(false);

  /** Toggle a node in the multi-selection set (Ctrl/Cmd+click). */
  toggleNode(nodeId: string) {
    const next = new Set(this.selectedNodeIds);
    if (next.has(nodeId)) {
      next.delete(nodeId);
    } else {
      next.add(nodeId);
    }
    this.selectedNodeIds = next;
    this.selectedNodeId = next.size > 0 ? nodeId : null;
  }

  /** Select a single node, replacing the multi-selection set. */
  selectSingle(nodeId: string) {
    this.selectedNodeIds = new Set([nodeId]);
    this.selectedNodeId = nodeId;
  }

  /** Remove a node from the multi-selection set. */
  deselectNode(nodeId: string) {
    const next = new Set(this.selectedNodeIds);
    next.delete(nodeId);
    this.selectedNodeIds = next;
    if (this.selectedNodeId === nodeId) {
      this.selectedNodeId = next.size > 0 ? [...next][0] : null;
    }
  }

  /** Clear selection and close panel. */
  clear() {
    this.selectedNodeId = null;
    this.selectedNodeIds = new Set();
    this.selectedNode = null;
    this.children = [];
    this.ancestors = [];
    this.dependencies = [];
    this.dependents = [];
    this.panelOpen = false;
  }
}

export const selectionStore = new SelectionStore();
