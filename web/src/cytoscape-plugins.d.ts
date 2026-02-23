declare module "cytoscape-cose-bilkent" {
  import type cytoscape from "cytoscape";
  const coseBilkent: cytoscape.Ext;
  export default coseBilkent;
}

declare module "cytoscape-dagre" {
  import type cytoscape from "cytoscape";
  const dagre: cytoscape.Ext;
  export default dagre;
}

declare module "cytoscape-fcose" {
  import type cytoscape from "cytoscape";
  const fcose: cytoscape.Ext;
  export default fcose;
}

declare module "cytoscape-elk" {
  import type cytoscape from "cytoscape";
  const elk: cytoscape.Ext;
  export default elk;
}

declare module "cytoscape-navigator" {
  import type cytoscape from "cytoscape";
  const navigator: cytoscape.Ext;
  export default navigator;
}

declare module "cytoscape-context-menus" {
  import type cytoscape from "cytoscape";
  interface ContextMenuItem {
    id: string;
    content: string;
    selector?: string;
    coreAsWell?: boolean;
    onClickFunction: (event: { target: cytoscape.SingularElementReturnValue }) => void;
    show?: boolean;
    hasTrailingDivider?: boolean;
  }
  interface ContextMenuOptions {
    menuItems: ContextMenuItem[];
    menuItemClasses?: string[];
    contextMenuClasses?: string[];
  }
  const contextMenus: cytoscape.Ext;
  export default contextMenus;
}

declare module "cytoscape-popper" {
  import type cytoscape from "cytoscape";
  const popper: cytoscape.Ext;
  export default popper;
}
