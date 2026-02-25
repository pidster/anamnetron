/** Reactive store for subtree focus navigation. */
class FocusStore {
  /** ID of the node at the center of focus, or null when inactive. */
  focusNodeId = $state<string | null>(null);
  /** History stack for back-navigation (most recent at end). */
  history = $state<string[]>([]);

  /** Whether focus mode is currently active. */
  get active(): boolean {
    return this.focusNodeId !== null;
  }

  /** Focus on a node's subtree. Pushes current focus to history. */
  focus(nodeId: string) {
    if (this.focusNodeId && this.focusNodeId !== nodeId) {
      this.history = [...this.history, this.focusNodeId];
    }
    this.focusNodeId = nodeId;
  }

  /** Navigate back one level in focus history. */
  back(): string | null {
    if (this.history.length === 0) {
      this.focusNodeId = null;
      return null;
    }
    const prev = this.history[this.history.length - 1];
    this.history = this.history.slice(0, -1);
    this.focusNodeId = prev;
    return prev;
  }

  /** Focus on a specific ancestor (clicking a breadcrumb). Truncates history. */
  focusAncestor(nodeId: string) {
    const idx = this.history.indexOf(nodeId);
    if (idx >= 0) {
      this.history = this.history.slice(0, idx);
    } else {
      this.history = [];
    }
    this.focusNodeId = nodeId;
  }

  /** Clear focus entirely, returning to full graph. */
  clear() {
    this.focusNodeId = null;
    this.history = [];
  }
}

export const focusStore = new FocusStore();
