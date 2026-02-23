/** Reactive store for scoped view (subgraph filtering). */
class ScopeStore {
  scopeNodeId = $state<string | null>(null);

  /** Set the scope to a specific node, showing only its subtree. */
  setScope(nodeId: string) {
    this.scopeNodeId = nodeId;
  }

  /** Clear the scope, restoring the full graph. */
  clear() {
    this.scopeNodeId = null;
  }

  /** Whether a scope is currently active. */
  get active(): boolean {
    return this.scopeNodeId !== null;
  }
}

export const scopeStore = new ScopeStore();
