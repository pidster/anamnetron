import type { NodeKind, EdgeKind, CyNodeData } from "../lib/types";
import { toggleInSet, extractSubKinds, extractLanguages, hasActiveFilters } from "../lib/filter-logic";

const ALL_NODE_KINDS: NodeKind[] = ["system", "service", "component", "unit"];
const ALL_EDGE_KINDS: EdgeKind[] = ["depends", "calls", "implements", "extends", "data_flow", "exports"];

/** Reactive store for graph filtering state. */
class FilterStore {
  nodeKinds = $state<Set<string>>(new Set(ALL_NODE_KINDS));
  edgeKinds = $state<Set<string>>(new Set(ALL_EDGE_KINDS));
  subKinds = $state<Set<string>>(new Set());
  languages = $state<Set<string>>(new Set());
  availableSubKinds = $state<string[]>([]);
  availableLanguages = $state<string[]>([]);
  testVisibility = $state<"all" | "code" | "tests">("all");
  sidebarOpen = $state(false);

  /** Toggle a node kind filter. */
  toggleNodeKind(kind: string) {
    this.nodeKinds = toggleInSet(this.nodeKinds, kind);
  }

  /** Toggle an edge kind filter. */
  toggleEdgeKind(kind: string) {
    this.edgeKinds = toggleInSet(this.edgeKinds, kind);
  }

  /** Toggle a sub-kind filter. */
  toggleSubKind(subKind: string) {
    this.subKinds = toggleInSet(this.subKinds, subKind);
  }

  /** Toggle a language filter. */
  toggleLanguage(lang: string) {
    this.languages = toggleInSet(this.languages, lang);
  }

  /** Set test visibility mode. */
  setTestVisibility(mode: "all" | "code" | "tests") {
    this.testVisibility = mode;
  }

  /** Populate available values from graph nodes and enable all by default. */
  populateFromGraph(nodes: Array<{ data: CyNodeData }>) {
    this.availableSubKinds = extractSubKinds(nodes);
    this.availableLanguages = extractLanguages(nodes);
    this.subKinds = new Set(this.availableSubKinds);
    this.languages = new Set(this.availableLanguages);
    this.nodeKinds = new Set(ALL_NODE_KINDS);
    this.edgeKinds = new Set(ALL_EDGE_KINDS);
  }

  /** Re-enable all filters. */
  resetAll() {
    this.nodeKinds = new Set(ALL_NODE_KINDS);
    this.edgeKinds = new Set(ALL_EDGE_KINDS);
    this.subKinds = new Set(this.availableSubKinds);
    this.languages = new Set(this.availableLanguages);
    this.testVisibility = "all";
  }

  /** Whether any filters are actively reducing the view. */
  get hasActiveFilters(): boolean {
    return hasActiveFilters({
      nodeKinds: this.nodeKinds,
      edgeKinds: this.edgeKinds,
      subKinds: this.subKinds,
      languages: this.languages,
      allNodeKinds: ALL_NODE_KINDS.length,
      allEdgeKinds: ALL_EDGE_KINDS.length,
      allSubKinds: this.availableSubKinds.length,
      allLanguages: this.availableLanguages.length,
      testVisibility: this.testVisibility,
    });
  }
}

export const filterStore = new FilterStore();
