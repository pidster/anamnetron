/** View mode types for the main visualisation area. */
export type ViewMode = "mermaid" | "graph" | "treemap" | "chord" | "sunburst";

/** Store for the currently active view mode. */
class ViewStore {
  mode = $state<ViewMode>("treemap");

  setMode(mode: ViewMode) {
    this.mode = mode;
  }
}

export const viewStore = new ViewStore();
