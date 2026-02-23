function loadSavedState(): { tab: "tree" | "filters"; collapsed: boolean } {
  if (typeof localStorage === "undefined") return { tab: "tree", collapsed: false };
  const tab = localStorage.getItem("svt-nav-tab");
  const collapsed = localStorage.getItem("svt-nav-collapsed");
  return {
    tab: tab === "filters" ? "filters" : "tree",
    collapsed: collapsed === "true",
  };
}

const saved = loadSavedState();

/** Reactive store for navigation panel state. */
class NavigationStore {
  activeTab = $state<"tree" | "filters">(saved.tab);
  collapsed = $state(saved.collapsed);

  /** Toggle the panel collapsed state. */
  toggle() {
    this.collapsed = !this.collapsed;
  }

  /** Expand the panel. */
  expand() {
    this.collapsed = false;
  }

  /** Collapse the panel. */
  collapse() {
    this.collapsed = true;
  }

  /** Switch to a tab (and expand if collapsed). */
  setTab(tab: "tree" | "filters") {
    this.activeTab = tab;
    if (this.collapsed) {
      this.collapsed = false;
    }
  }
}

export const navigationStore = new NavigationStore();
