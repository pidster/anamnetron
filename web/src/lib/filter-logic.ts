import type { CyNodeData } from "./types";

/** Toggle an item's membership in a set, returning a new set. */
export function toggleInSet<T>(set: Set<T>, item: T): Set<T> {
  const next = new Set(set);
  if (next.has(item)) {
    next.delete(item);
  } else {
    next.add(item);
  }
  return next;
}

/** Extract unique sorted sub-kinds from graph nodes. */
export function extractSubKinds(nodes: Array<{ data: CyNodeData }>): string[] {
  const kinds = new Set<string>();
  for (const node of nodes) {
    if (node.data.sub_kind) {
      kinds.add(node.data.sub_kind);
    }
  }
  return [...kinds].sort();
}

/** Extract unique sorted languages from graph nodes. */
export function extractLanguages(nodes: Array<{ data: CyNodeData }>): string[] {
  const langs = new Set<string>();
  for (const node of nodes) {
    if (node.data.language) {
      langs.add(node.data.language);
    }
  }
  return [...langs].sort();
}

/** Check whether any filters are actively reducing the view. */
export interface FilterState {
  nodeKinds: Set<string>;
  edgeKinds: Set<string>;
  subKinds: Set<string>;
  languages: Set<string>;
  allNodeKinds: number;
  allEdgeKinds: number;
  allSubKinds: number;
  allLanguages: number;
}

/** Returns true if any filter dimension has items disabled. */
export function hasActiveFilters(state: FilterState): boolean {
  return (
    state.nodeKinds.size < state.allNodeKinds ||
    state.edgeKinds.size < state.allEdgeKinds ||
    state.subKinds.size < state.allSubKinds ||
    state.languages.size < state.allLanguages
  );
}

/** Format a snake_case or lowercase string into Title Case. */
export function formatLabel(value: string): string {
  return value
    .split("_")
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}
