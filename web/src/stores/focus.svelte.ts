/** Reactive store for neighborhood focus mode. */
class FocusStore {
  /** ID of the node at the center of focus, or null when inactive. */
  focusNodeId = $state<string | null>(null);
  /** Number of edge hops to include in the focused neighborhood. */
  focusDegrees = $state(1);

  /** Whether focus mode is currently active. */
  get active(): boolean {
    return this.focusNodeId !== null;
  }

  /** Focus on a specific node's neighborhood. */
  focus(nodeId: string) {
    this.focusNodeId = nodeId;
  }

  /** Clear focus mode. */
  clear() {
    this.focusNodeId = null;
  }

  /** Toggle focus on a node — if already focused on it, clear; otherwise focus. */
  toggle(nodeId: string) {
    if (this.focusNodeId === nodeId) {
      this.clear();
    } else {
      this.focus(nodeId);
    }
  }
}

export const focusStore = new FocusStore();
