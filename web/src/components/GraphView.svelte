<script lang="ts">
  import { onMount } from "svelte";
  import cytoscape from "cytoscape";
  import dagre from "cytoscape-dagre";
  import fcose from "cytoscape-fcose";
  import elk from "cytoscape-elk";
  import navigator from "cytoscape-navigator";
  import contextMenus from "cytoscape-context-menus";
  import popper from "cytoscape-popper";
  import tippy, { type Instance as TippyInstance } from "tippy.js";
  import type { CytoscapeGraph, ConformanceReport, SnapshotDiff, LayoutType } from "../lib/types";
  import { selectionStore } from "../stores/selection.svelte";
  import { buildTraversalIndex, type TraversalIndex } from "../lib/traversal";
  import { computeVisibleElements } from "../lib/expansion";
  import { KIND_COLORS, SUB_KIND_SHAPES, EDGE_STYLES } from "../lib/visual-encoding";

  // Register layout and interaction extensions once
  cytoscape.use(dagre);
  cytoscape.use(fcose);
  cytoscape.use(elk);
  cytoscape.use(navigator);
  cytoscape.use(contextMenus);
  cytoscape.use(popper);

  interface Props {
    graph: CytoscapeGraph | null;
    expandedNodes?: Set<string>;
    onToggleExpand?: (nodeId: string) => void;
    onFocusNode?: (nodeId: string) => void;
    conformance?: ConformanceReport | null;
    diff?: SnapshotDiff | null;
    layout?: LayoutType;
    theme?: "dark" | "light";
    filterNodeKinds?: Set<string>;
    filterEdgeKinds?: Set<string>;
    filterSubKinds?: Set<string>;
    filterLanguages?: Set<string>;
    focusNodeId?: string | null;
    focusDegrees?: number;
  }

  let {
    graph,
    expandedNodes,
    onToggleExpand,
    onFocusNode,
    conformance = null,
    diff = null,
    layout = "fcose",
    theme = "dark",
    filterNodeKinds,
    filterEdgeKinds,
    filterSubKinds,
    filterLanguages,
    focusNodeId = null,
    focusDegrees = 1,
  }: Props = $props();

  let container: HTMLDivElement;
  let cy: cytoscape.Core | null = null;
  let traversalIndex: TraversalIndex | null = null;
  let pathIndex: Map<string, cytoscape.NodeSingular> = new Map();
  let tapTimeout: ReturnType<typeof setTimeout> | null = null;
  let activeTippy: TippyInstance | null = null;

  function getCssVar(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }

  function buildStyleSheet(): cytoscape.StylesheetStyle[] {
    const accent = getCssVar("--accent") || "#53a8b6";
    const muted = getCssVar("--muted") || "#607d8b";
    const pass = getCssVar("--pass") || "#4caf50";
    const fail = getCssVar("--fail") || "#f44336";
    const warn = getCssVar("--warn") || "#ff9800";
    const text = getCssVar("--text") || "#e0e0e0";
    const isDark = theme === "dark";
    const parentBg = isDark ? "#16213e" : "#e8eef3";
    const parentBorder = isDark ? "#0f3460" : "#b0c4d8";
    const selectedBorder = isDark ? "#fff" : "#000";

    const styles: cytoscape.StylesheetStyle[] = [
      // Base node style
      {
        selector: "node",
        style: {
          label: "data(label)",
          "text-valign": "center",
          "text-halign": "center",
          "background-color": accent,
          color: "#fff",
          "font-size": "12px",
          "text-wrap": "wrap",
          "text-max-width": "80px",
          width: "label",
          height: "label",
          padding: "10px",
          shape: "roundrectangle",
          "transition-property": "opacity",
          "transition-duration": 200,
        },
      },
      // Parent (compound) node style
      {
        selector: "node:parent",
        style: {
          "background-color": parentBg,
          "background-opacity": 0.6,
          "border-color": parentBorder,
          "border-width": 2,
          "text-valign": "top",
          "text-halign": "center",
          color: text,
          "font-size": "14px",
          "font-weight": "bold",
          padding: "20px",
        },
      },
      // Collapsed parent (leaf with children)
      {
        selector: "node[_childCount]",
        style: {
          "border-style": "double",
          "border-width": 4,
          "border-color": parentBorder,
          label: (ele: cytoscape.NodeSingular) =>
            `${ele.data("label")} (${ele.data("_childCount")})`,
        },
      },
    ];

    // Node kind colors (leaf nodes)
    for (const [kind, cssVarName] of Object.entries(KIND_COLORS)) {
      const color = getCssVar(cssVarName) || accent;
      styles.push({
        selector: `node[kind = '${kind}']:childless`,
        style: { "background-color": color },
      });
      // Parent border color per kind
      styles.push({
        selector: `node:parent[kind = '${kind}']`,
        style: { "border-color": color },
      });
    }

    // Sub-kind shapes (leaf nodes only — compound parents must stay rectangular)
    for (const [subKind, shape] of Object.entries(SUB_KIND_SHAPES)) {
      styles.push({
        selector: `node[sub_kind = '${subKind}']:childless`,
        style: { shape },
      });
    }

    // Base edge style
    styles.push({
      selector: "edge",
      style: {
        width: 2,
        "line-color": muted,
        "target-arrow-color": muted,
        "target-arrow-shape": "triangle",
        "curve-style": "bezier",
        "arrow-scale": 0.8,
        "transition-property": "opacity",
        "transition-duration": 200,
      },
    });

    // Edge kind styles (all 6 non-contains kinds)
    for (const [kind, def] of Object.entries(EDGE_STYLES)) {
      const color = getCssVar(def.cssVar) || accent;
      styles.push({
        selector: `edge[kind = '${kind}']`,
        style: {
          "line-style": def.lineStyle,
          "line-color": color,
          "target-arrow-color": color,
          "target-arrow-shape": def.arrowShape,
        },
      });
    }

    // Meta-edge style
    styles.push({
      selector: "edge[_isMeta]",
      style: {
        "line-style": "dashed",
        width: 4,
        label: "data(_count)",
        "font-size": "10px",
        "text-background-color": isDark ? "#132f4c" : "#ffffff",
        "text-background-opacity": 0.8,
        "text-background-padding": "2px",
        color: text,
      },
    });

    // Hover/focus highlight classes
    styles.push(
      {
        selector: ".faded",
        style: { opacity: 0.15 },
      },
      {
        selector: ".highlighted",
        style: { opacity: 1, "z-index": 10 },
      },
    );

    // Selection, conformance, and diff styles (unchanged from original)
    styles.push(
      {
        selector: "node:selected",
        style: { "border-color": selectedBorder, "border-width": 3 },
      },
      {
        selector: ".conformance-pass",
        style: { "border-color": pass, "border-width": 3 },
      },
      {
        selector: ".conformance-fail",
        style: { "border-color": fail, "border-width": 3 },
      },
      {
        selector: ".conformance-unimplemented",
        style: { "border-color": warn, "border-width": 3 },
      },
      {
        selector: ".conformance-undocumented",
        style: { "border-color": muted, "border-width": 3 },
      },
      {
        selector: ".diff-added",
        style: { "border-color": pass, "border-width": 3, "border-style": "dashed" },
      },
      {
        selector: ".diff-removed",
        style: { "border-color": fail, "border-width": 3, "border-style": "dashed", opacity: 0.5 },
      },
      {
        selector: ".diff-changed",
        style: { "border-color": warn, "border-width": 3, "border-style": "dashed" },
      },
      {
        selector: "edge.diff-added",
        style: { "line-color": pass, "target-arrow-color": pass, "line-style": "dashed" },
      },
      {
        selector: "edge.diff-removed",
        style: { "line-color": fail, "target-arrow-color": fail, "line-style": "dashed", opacity: 0.5 },
      },
    );

    return styles;
  }

  function destroyTooltip() {
    if (activeTippy) {
      activeTippy.destroy();
      activeTippy = null;
    }
  }

  function escapeHtml(str: string): string {
    return str.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  }

  function showTooltip(node: cytoscape.NodeSingular) {
    destroyTooltip();
    if (!cy) return;

    const ref = node.popperRef();
    const content = document.createElement("div");
    const label = escapeHtml(String(node.data("label") || node.id()));
    const kind = escapeHtml(String(node.data("kind") || ""));
    const subKind = escapeHtml(String(node.data("sub_kind") || ""));
    const cp = escapeHtml(String(node.data("canonical_path") || ""));
    const lang = escapeHtml(String(node.data("language") || ""));
    const childCount = node.data("_childCount");

    let html = `<strong>${label}</strong>`;
    if (kind) html += `<br/>${kind}`;
    if (subKind) html += ` / ${subKind}`;
    if (cp) html += `<br/><code>${cp}</code>`;
    if (lang) html += `<br/>Language: ${lang}`;
    if (childCount !== undefined) html += `<br/>${escapeHtml(String(childCount))} descendants`;
    content.innerHTML = html;

    activeTippy = tippy(document.createElement("div"), {
      getReferenceClientRect: ref.getBoundingClientRect,
      content,
      allowHTML: true,
      placement: "top",
      arrow: true,
      theme: theme === "dark" ? "dark-tooltip" : "light-tooltip",
      appendTo: container,
      showOnCreate: true,
      interactive: false,
      trigger: "manual",
    });
  }

  function applyFocusDimming(targetNodeId: string, degrees: number): cytoscape.Collection | null {
    if (!cy) return null;
    const node = cy.getElementById(targetNodeId);
    if (node.length === 0) return null;
    let extended = node.closedNeighborhood();
    for (let i = 1; i < degrees; i++) {
      extended = extended.closedNeighborhood();
    }
    cy.startBatch();
    cy.elements().addClass("faded").removeClass("highlighted");
    extended.removeClass("faded").addClass("highlighted");
    cy.endBatch();
    return extended;
  }

  function clearFocusDimming() {
    if (!cy) return;
    cy.elements().removeClass("faded highlighted");
  }

  function getLayoutConfig(name: LayoutType, nodeCount: number): cytoscape.LayoutOptions {
    const base = {
      animate: nodeCount >= 200,
      nodeDimensionsIncludeLabels: true,
    };
    switch (name) {
      case "elk":
        return {
          name: "elk",
          ...base,
          elk: {
            algorithm: "layered",
            "elk.direction": "DOWN",
            "elk.spacing.nodeNode": "50",
            "elk.layered.spacing.nodeNodeBetweenLayers": "70",
          },
        } as unknown as cytoscape.LayoutOptions;
      case "dagre":
        return { name: "dagre", ...base } as cytoscape.LayoutOptions;
      case "fcose":
      default:
        return { name: "fcose", ...base, quality: "proof" } as unknown as cytoscape.LayoutOptions;
    }
  }

  function initCytoscape(elements: CytoscapeGraph["elements"]) {
    if (cy) {
      destroyTooltip();
      cy.destroy();
    }
    pathIndex.clear();

    const nodeCount = elements.nodes.length;

    cy = cytoscape({
      container,
      elements: {
        nodes: elements.nodes,
        edges: elements.edges,
      },
      style: buildStyleSheet(),
      layout: getLayoutConfig(layout, nodeCount),
      textureOnViewport: nodeCount > 300,
      hideEdgesOnViewport: nodeCount > 500,
    } as cytoscape.CytoscapeOptions);

    // --- Tap / double-tap with race-condition fix ---
    cy.on("tap", "node", (evt) => {
      const nodeId = evt.target.id();
      // Delay selection to allow dbltap to cancel it
      if (tapTimeout) clearTimeout(tapTimeout);
      tapTimeout = setTimeout(() => {
        tapTimeout = null;
        selectionStore.selectedNodeId = nodeId;
        selectionStore.panelOpen = true;
      }, 250);
    });

    cy.on("tap", (evt) => {
      if (evt.target === cy) {
        if (tapTimeout) {
          clearTimeout(tapTimeout);
          tapTimeout = null;
        }
        selectionStore.clear();
      }
    });

    cy.on("dbltap", "node", (evt) => {
      // Cancel the pending tap selection
      if (tapTimeout) {
        clearTimeout(tapTimeout);
        tapTimeout = null;
      }
      const nodeId = evt.target.id();
      if (onToggleExpand && traversalIndex?.childrenMap.has(nodeId)) {
        onToggleExpand(nodeId);
      }
    });

    // --- Hover: neighborhood highlighting + tooltips ---
    cy.on("mouseover", "node", (evt) => {
      const node = evt.target as cytoscape.NodeSingular;
      if (!cy) return;

      // Show tooltip
      showTooltip(node);

      // Highlight neighborhood
      const neighborhood = node.closedNeighborhood();
      cy.startBatch();
      cy.elements().addClass("faded");
      neighborhood.removeClass("faded").addClass("highlighted");
      cy.endBatch();
    });

    cy.on("mouseout", "node", () => {
      destroyTooltip();
      if (!cy) return;
      cy.startBatch();
      cy.elements().removeClass("faded highlighted");
      cy.endBatch();

      // Re-apply focus dimming if focus mode is active
      if (focusNodeId) {
        applyFocusDimming(focusNodeId, focusDegrees);
      }
    });

    // --- Context menu ---
    const menuItems = [
      {
        id: "expand-collapse",
        content: "Expand/Collapse",
        selector: "node",
        onClickFunction: (event: { target: cytoscape.SingularElementReturnValue }) => {
          const nodeId = event.target.id();
          if (onToggleExpand && traversalIndex?.childrenMap.has(nodeId)) {
            onToggleExpand(nodeId);
          }
        },
      },
      {
        id: "focus-neighborhood",
        content: "Focus Neighborhood",
        selector: "node",
        onClickFunction: (event: { target: cytoscape.SingularElementReturnValue }) => {
          const nodeId = event.target.id();
          if (onFocusNode) onFocusNode(nodeId);
        },
      },
      {
        id: "copy-path",
        content: "Copy Canonical Path",
        selector: "node",
        onClickFunction: (event: { target: cytoscape.SingularElementReturnValue }) => {
          const cp = event.target.data("canonical_path") as string;
          if (cp) void globalThis.navigator.clipboard?.writeText(cp).catch(() => {});
        },
      },
      {
        id: "fit-selection",
        content: "Fit to Neighborhood",
        selector: "node",
        onClickFunction: (event: { target: cytoscape.SingularElementReturnValue }) => {
          if (!cy) return;
          const neighborhood = (event.target as cytoscape.NodeSingular).closedNeighborhood();
          cy.fit(neighborhood, 50);
        },
      },
    ];

    (cy as unknown as { contextMenus: (opts: unknown) => void }).contextMenus({
      menuItems,
      menuItemClasses: ["cy-context-menu-item"],
      contextMenuClasses: ["cy-context-menu"],
    });

    // --- Minimap ---
    const navContainer = container.querySelector(".cy-navigator");
    if (navContainer) {
      (cy as unknown as { navigator: (opts: unknown) => unknown }).navigator({
        container: navContainer,
        viewLiveFramerate: 0,
        thumbnailEventFramerate: 10,
        thumbnailLiveFramerate: 0,
        dblClickDelay: 200,
      });
    }

    // Build canonical path index for O(1) lookups in overlays
    cy.nodes().forEach((node) => {
      const cp = node.data("canonical_path") as string | undefined;
      if (cp) pathIndex.set(cp, node);
    });
  }

  function applyConformanceOverlay(report: ConformanceReport) {
    if (!cy) return;

    // Clear previous overlay
    cy.nodes().removeClass(
      "conformance-pass conformance-fail conformance-unimplemented conformance-undocumented",
    );

    // Mark failed constraints via path index
    for (const result of report.constraint_results) {
      if (result.status === "fail") {
        for (const violation of result.violations) {
          const node = pathIndex.get(violation.source_path);
          if (node) node.addClass("conformance-fail");
        }
      }
    }

    // Mark unimplemented via path index
    for (const entry of report.unimplemented) {
      const node = pathIndex.get(entry.canonical_path);
      if (node) node.addClass("conformance-unimplemented");
    }

    // Mark undocumented via path index
    for (const entry of report.undocumented) {
      const node = pathIndex.get(entry.canonical_path);
      if (node) node.addClass("conformance-undocumented");
    }

    // Remaining nodes with no overlay = pass
    cy.nodes()
      .filter(
        (n) =>
          !n.hasClass("conformance-fail") &&
          !n.hasClass("conformance-unimplemented") &&
          !n.hasClass("conformance-undocumented"),
      )
      .addClass("conformance-pass");
  }

  function applyDiffOverlay(report: SnapshotDiff) {
    if (!cy) return;

    // Clear previous diff overlay
    cy.elements().removeClass("diff-added diff-removed diff-changed");

    // Apply node changes via path index
    for (const change of report.node_changes) {
      const node = pathIndex.get(change.canonical_path);
      if (node) node.addClass(`diff-${change.change}`);
    }

    // Build composite edge key map for O(1) lookups
    const edgeIndex = new Map<string, cytoscape.EdgeSingular>();
    cy.edges().forEach((edge) => {
      const srcPath = cy!.getElementById(edge.data("source")).data("canonical_path") as string;
      const tgtPath = cy!.getElementById(edge.data("target")).data("canonical_path") as string;
      const kind = edge.data("kind") as string;
      if (srcPath && tgtPath && kind) {
        edgeIndex.set(`${srcPath}\0${tgtPath}\0${kind}`, edge);
      }
    });

    // Apply edge changes via composite key lookup
    for (const change of report.edge_changes) {
      const key = `${change.source_path}\0${change.target_path}\0${change.edge_kind}`;
      const edge = edgeIndex.get(key);
      if (edge) edge.addClass(`diff-${change.change}`);
    }
  }

  function clearDiffOverlay() {
    if (!cy) return;
    cy.elements().removeClass("diff-added diff-removed diff-changed");
  }

  onMount(() => {
    const resizeObserver = new ResizeObserver(() => {
      if (cy) cy.resize();
    });
    resizeObserver.observe(container);
    return () => {
      resizeObserver.disconnect();
      if (tapTimeout) {
        clearTimeout(tapTimeout);
        tapTimeout = null;
      }
      destroyTooltip();
      if (cy) cy.destroy();
    };
  });

  // Rebuild traversal index when the full graph changes
  $effect(() => {
    if (graph) {
      traversalIndex = buildTraversalIndex(graph);
    } else {
      traversalIndex = null;
    }
  });

  // Compute visible subset based on expansion state
  let visibleGraph = $derived.by(() => {
    if (!graph || !traversalIndex || !expandedNodes) return graph;
    return computeVisibleElements(graph, expandedNodes, traversalIndex);
  });

  $effect(() => {
    if (visibleGraph && container) {
      initCytoscape(visibleGraph.elements);
    }
  });

  $effect(() => {
    if (conformance && cy) {
      applyConformanceOverlay(conformance);
    }
  });

  $effect(() => {
    if (diff && cy) {
      applyDiffOverlay(diff);
    } else if (!diff && cy) {
      clearDiffOverlay();
    }
  });

  // Re-apply styles when theme changes
  $effect(() => {
    // Access theme to track it as a dependency
    const _ = theme;
    if (cy) {
      cy.style(buildStyleSheet() as unknown as cytoscape.StylesheetCSS[]);
    }
  });

  // Apply focus mode dimming
  $effect(() => {
    const fid = focusNodeId;
    const deg = focusDegrees;
    if (!cy) return;
    if (fid) {
      const neighborhood = applyFocusDimming(fid, deg);
      if (neighborhood) {
        cy.animate({ fit: { eles: neighborhood, padding: 50 }, duration: 300 });
      }
    } else {
      clearFocusDimming();
    }
  });

  // Apply filters when filter state changes
  $effect(() => {
    if (!cy) return;
    // Access all filter props to track as dependencies
    const nk = filterNodeKinds;
    const ek = filterEdgeKinds;
    const sk = filterSubKinds;
    const lg = filterLanguages;
    if (!nk || !ek || !sk || !lg) return;

    cy.startBatch();

    // Single-pass filter: only toggle visibility when state actually changes
    cy.nodes().forEach((node) => {
      const kind = node.data("kind") as string;
      const subKind = node.data("sub_kind") as string;
      const language = node.data("language") as string | undefined;

      const kindMatch = nk.has(kind);
      const subKindMatch = !subKind || sk.has(subKind);
      const langMatch = !language || lg.has(language);
      const shouldShow = kindMatch && subKindMatch && langMatch;

      if (shouldShow && node.hidden()) {
        node.show();
      } else if (!shouldShow && node.visible()) {
        node.hide();
      }
    });

    cy.edges().forEach((edge) => {
      const kind = edge.data("kind") as string;
      const shouldShow = ek.has(kind) && edge.source().visible() && edge.target().visible();

      if (shouldShow && edge.hidden()) {
        edge.show();
      } else if (!shouldShow && edge.visible()) {
        edge.hide();
      }
    });

    cy.endBatch();
  });

  /** Select and center on a node. */
  export function selectAndCenter(nodeId: string) {
    if (!cy) return;
    const node = cy.getElementById(nodeId);
    if (node.length === 0) return;
    cy.animate({ center: { eles: node }, duration: 200 });
    selectionStore.selectedNodeId = nodeId;
    selectionStore.panelOpen = true;
  }

  /** Fit all elements in viewport. */
  export function fitAll() {
    if (!cy) return;
    cy.fit(undefined, 50);
  }

  /** Get the current traversal index for keyboard navigation. */
  export function getTraversalIndex(): TraversalIndex | null {
    return traversalIndex;
  }

  /** Re-run layout. */
  export function relayout(name?: LayoutType) {
    if (!cy) return;
    const layoutName = name || layout;
    const nodeCount = cy.nodes().length;
    cy.layout(getLayoutConfig(layoutName, nodeCount)).run();
  }
</script>

<div class="graph-container" bind:this={container}>
  <div class="cy-navigator"></div>
</div>

<style>
  .graph-container {
    flex: 1;
    min-height: 0;
    background: var(--bg);
    position: relative;
  }

  .cy-navigator {
    position: absolute;
    bottom: 10px;
    right: 10px;
    width: 180px;
    height: 120px;
    border: 1px solid var(--border);
    background: var(--surface);
    opacity: 0.85;
    z-index: 10;
    overflow: hidden;
  }

  /* Context menu theming */
  :global(.cy-context-menu) {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 0;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    z-index: 1000;
  }

  :global(.cy-context-menu-item) {
    padding: 6px 16px;
    color: var(--text);
    font-size: 0.85rem;
    cursor: pointer;
  }

  :global(.cy-context-menu-item:hover) {
    background: var(--accent);
    color: #fff;
  }

  /* Tooltip theming */
  :global(.tippy-box[data-theme~='dark-tooltip']) {
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--border);
    font-size: 0.8rem;
    line-height: 1.4;
  }

  :global(.tippy-box[data-theme~='dark-tooltip'] .tippy-arrow) {
    color: var(--surface);
  }

  :global(.tippy-box[data-theme~='light-tooltip']) {
    background: #fff;
    color: #1f2328;
    border: 1px solid #d0d7de;
    font-size: 0.8rem;
    line-height: 1.4;
  }

  :global(.tippy-box[data-theme~='light-tooltip'] .tippy-arrow) {
    color: #fff;
  }

  :global(.tippy-box code) {
    font-size: 0.75rem;
    color: var(--accent);
  }
</style>
