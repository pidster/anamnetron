import type { TraversalIndex } from "../lib/traversal";
import { computeDefaultExpansion, getAncestorChain } from "../lib/expansion";

/** Reactive store tracking which parent nodes are expanded. */
class ExpansionStore {
  expandedNodes = $state<Set<string>>(new Set());
  currentDepth = $state(2);

  /** Toggle a node between expanded and collapsed. */
  toggle(nodeId: string) {
    const next = new Set(this.expandedNodes);
    if (next.has(nodeId)) {
      next.delete(nodeId);
    } else {
      next.add(nodeId);
    }
    this.expandedNodes = next;
  }

  /** Expand a single node. */
  expand(nodeId: string) {
    if (this.expandedNodes.has(nodeId)) return;
    const next = new Set(this.expandedNodes);
    next.add(nodeId);
    this.expandedNodes = next;
  }

  /** Collapse a single node. */
  collapse(nodeId: string) {
    if (!this.expandedNodes.has(nodeId)) return;
    const next = new Set(this.expandedNodes);
    next.delete(nodeId);
    this.expandedNodes = next;
  }

  /** Check if a node is expanded. */
  isExpanded(nodeId: string): boolean {
    return this.expandedNodes.has(nodeId);
  }

  /** Replace the expansion set with all nodes at or above the given depth. */
  expandToDepth(depth: number, index: TraversalIndex) {
    this.currentDepth = depth;
    this.expandedNodes = computeDefaultExpansion(index, depth);
  }

  /** Expand all ancestors of a node so it becomes visible. */
  expandAncestors(nodeId: string, index: TraversalIndex) {
    const chain = getAncestorChain(index, nodeId);
    if (chain.length === 0) return;
    const next = new Set(this.expandedNodes);
    for (const ancestor of chain) {
      next.add(ancestor);
    }
    this.expandedNodes = next;
  }

  /** Collapse everything. */
  collapseAll() {
    this.currentDepth = 0;
    this.expandedNodes = new Set();
  }
}

export const expansionStore = new ExpansionStore();
