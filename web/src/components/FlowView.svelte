<script lang="ts">
  import { onMount } from "svelte";
  import cytoscape from "cytoscape";
  import fcose from "cytoscape-fcose";
  import type { CytoscapeGraph } from "../lib/types";
  import { EDGE_STYLES, KIND_COLORS } from "../lib/visual-encoding";
  import { flowStore } from "../stores/flow.svelte";
  import { graphStore } from "../stores/graph.svelte";
  import { buildFlowElements, computeClientRoots, type FlowNode, type FlowEdge } from "../lib/flow-layout";
  import { startAnimation } from "../lib/flow-animation";
  import * as api from "../lib/api";

  // Register fcose layout
  cytoscape.use(fcose);

  interface Props {
    graph: CytoscapeGraph | null;
    onselectnode?: (nodeId: string) => void;
  }

  let { graph, onselectnode }: Props = $props();

  let containerEl = $state<HTMLDivElement>(undefined!);
  let cy: cytoscape.Core | null = null;
  let stopAnimation: (() => void) | null = null;

  // Tooltip state
  let tooltip = $state<{
    visible: boolean;
    x: number;
    y: number;
    label: string;
    kind: string;
    subKind: string;
    childCount: number;
  }>({ visible: false, x: 0, y: 0, label: "", kind: "", subKind: "", childCount: 0 });

  // Selected node for highlighting
  let selectedNodeId = $state<string | null>(null);

  function getCssVar(name: string): string {
    return getComputedStyle(document.documentElement)
      .getPropertyValue(name)
      .trim();
  }

  /** Build Cytoscape stylesheet from visual encoding config. */
  function buildStylesheet(): cytoscape.StylesheetStyle[] {
    const accentColor = getCssVar("--accent") || "#5b9bd5";
    const textColor = getCssVar("--text") || "#e0e0e0";
    const surfaceColor = getCssVar("--surface") || "#1e1e2e";
    const borderColor = getCssVar("--border") || "#444";

    const styles: cytoscape.StylesheetStyle[] = [
      // Base node style
      {
        selector: "node",
        style: {
          label: "data(label)",
          "text-valign": "center",
          "text-halign": "center",
          "font-size": "10px",
          color: textColor,
          "text-outline-color": surfaceColor,
          "text-outline-width": 1.5,
          "background-color": accentColor,
          width: 30,
          height: 30,
          "border-width": 1,
          "border-color": borderColor,
        },
      },
      // Compound node (parent) style
      {
        selector: "node:parent",
        style: {
          "background-opacity": 0.08,
          "background-color": accentColor,
          "border-width": 1,
          "border-color": borderColor,
          "border-opacity": 0.4,
          label: "data(label)",
          "text-valign": "top",
          "text-halign": "center",
          "font-size": "11px",
          "font-weight": "bold",
          padding: "12px",
        } as cytoscape.Css.Node,
      },
      // Collapsed node
      {
        selector: "node.collapsed",
        style: {
          shape: "roundrectangle" as cytoscape.Css.NodeShape,
          width: 40,
          height: 40,
          "border-style": "dashed",
          "border-width": 2,
        },
      },
      // Root nodes — larger, bold border
      {
        selector: "node.root",
        style: {
          width: 38,
          height: 38,
          "border-width": 2.5,
          "border-color": accentColor,
        },
      },
      // Dimmed state for non-connected nodes during selection
      {
        selector: "node.dimmed",
        style: {
          opacity: 0.2,
        },
      },
      // Highlighted (connected to selection)
      {
        selector: "node.highlighted",
        style: {
          opacity: 1,
          "border-width": 2,
          "border-color": accentColor,
        },
      },
      // Selected node glow
      {
        selector: "node.selected-glow",
        style: {
          "border-width": 3,
          "border-color": accentColor,
          "overlay-color": accentColor,
          "overlay-padding": 4,
          "overlay-opacity": 0.15,
        },
      },
    ];

    // Node kind colors
    for (const [kind, cssVar] of Object.entries(KIND_COLORS)) {
      const color = getCssVar(cssVar) || accentColor;
      styles.push({
        selector: `node.kind-${kind}`,
        style: { "background-color": color },
      });
    }

    // Base edge style
    styles.push({
      selector: "edge",
      style: {
        width: 1.5,
        "curve-style": "bezier",
        "target-arrow-shape": "triangle",
        "target-arrow-color": borderColor,
        "line-color": borderColor,
        "arrow-scale": 0.7,
        opacity: 0.6,
      },
    });

    // Edge kind styles
    for (const [kind, edgeStyle] of Object.entries(EDGE_STYLES)) {
      const color = getCssVar(edgeStyle.cssVar) || accentColor;
      styles.push({
        selector: `edge[kind="${kind}"]`,
        style: {
          "line-color": color,
          "target-arrow-color": color,
          "line-style": edgeStyle.lineStyle as "solid" | "dashed" | "dotted",
          "target-arrow-shape": edgeStyle.arrowShape as cytoscape.Css.ArrowShape,
          width: kind === "calls" || kind === "data_flow" ? 2 : 1.2,
        },
      });
    }

    // Dimmed edges
    styles.push({
      selector: "edge.dimmed",
      style: { opacity: 0.08 },
    });

    // Highlighted edges
    styles.push({
      selector: "edge.highlighted",
      style: { opacity: 1, width: 2.5 },
    });

    return styles;
  }

  /** Initialize or update the Cytoscape instance. */
  function initCytoscape(elements: { nodes: FlowNode[]; edges: FlowEdge[] }) {
    if (cy) {
      stopAnimation?.();
      stopAnimation = null;
      cy.destroy();
      cy = null;
    }

    if (!containerEl || elements.nodes.length === 0) return;

    // Collect root IDs for alignment constraints
    const rootIds: string[] = [];
    const sinkIds: string[] = [];
    for (const n of elements.nodes as Array<{ data: { rootCategory?: string; id: string } }>) {
      if (n.data.rootCategory === "call_tree_root" || n.data.rootCategory === "containment_root") {
        rootIds.push(n.data.id);
      }
      if (n.data.rootCategory === "leaf_sink") {
        sinkIds.push(n.data.id);
      }
    }

    // Strip compound parents for flat layout when there are many nodes
    // to avoid fcose stack overflow on deeply nested compound graphs
    const flattenParents = elements.nodes.length > 150;
    const layoutNodes = flattenParents
      ? elements.nodes.map((n) => {
          const { parent: _, ...rest } = n.data;
          return { ...n, data: rest };
        })
      : elements.nodes;

    let instance: cytoscape.Core;
    try {
      instance = cytoscape({
        container: containerEl,
        elements: {
          nodes: layoutNodes as cytoscape.NodeDefinition[],
          edges: elements.edges as cytoscape.EdgeDefinition[],
        },
        style: buildStylesheet(),
        layout: {
          name: "fcose",
          quality: elements.nodes.length > 100 ? "draft" : "default",
          animate: false,
          nodeRepulsion: () => 4500,
          idealEdgeLength: () => 100,
          edgeElasticity: () => 0.45,
          gravity: 0.25,
          gravityRange: 3.8,
          nodeSeparation: 75,
          // Pin roots toward top, sinks toward bottom
          alignmentConstraint: {
            vertical: rootIds.length > 0 ? [rootIds] : undefined,
          },
          fixedNodeConstraint: undefined,
          relativePlacementConstraint: rootIds.length > 0 && sinkIds.length > 0
            ? rootIds.flatMap((r) =>
                sinkIds.map((s) => ({ top: r, bottom: s, gap: 200 }))
              ).slice(0, 20) // Cap constraints to avoid layout explosion
            : undefined,
        } as cytoscape.LayoutOptions,
        minZoom: 0.15,
        maxZoom: 4,
        wheelSensitivity: 0.3,
      });
    } catch (err) {
      // fcose can overflow on complex compound graphs — fall back to cose
      console.warn("[FlowView] fcose layout failed, falling back to cose:", err);
      instance = cytoscape({
        container: containerEl,
        elements: {
          nodes: layoutNodes as cytoscape.NodeDefinition[],
          edges: elements.edges as cytoscape.EdgeDefinition[],
        },
        style: buildStylesheet(),
        layout: { name: "cose", animate: false } as cytoscape.LayoutOptions,
        minZoom: 0.15,
        maxZoom: 4,
        wheelSensitivity: 0.3,
      });
    }

    // Node tap — select and highlight subgraph
    instance.on("tap", "node", (evt) => {
      const node = evt.target;
      const nodeId = node.id();

      // Handle double-tap for collapsed node expansion
      if (node.hasClass("collapsed")) {
        flowStore.toggleNode(nodeId);
        return;
      }

      selectedNodeId = nodeId;
      onselectnode?.(nodeId);
      highlightConnected(instance, nodeId);
    });

    // Tap on background — clear selection
    instance.on("tap", (evt) => {
      if (evt.target === instance) {
        clearHighlights(instance);
        selectedNodeId = null;
      }
    });

    // Double-tap on collapsed node — expand
    instance.on("dbltap", "node.collapsed", (evt) => {
      flowStore.toggleNode(evt.target.id());
    });

    // Mouseover/out for tooltip
    instance.on("mouseover", "node", (evt) => {
      const node = evt.target;
      const pos = evt.renderedPosition ?? evt.position;
      const containerRect = containerEl.getBoundingClientRect();
      tooltip = {
        visible: true,
        x: containerRect.left + (pos?.x ?? 0),
        y: containerRect.top + (pos?.y ?? 0),
        label: node.data("label") ?? node.id(),
        kind: node.data("kind") ?? "",
        subKind: node.data("sub_kind") ?? "",
        childCount: node.data("childCount") ?? 0,
      };
    });

    instance.on("mouseout", "node", () => {
      tooltip = { ...tooltip, visible: false };
    });

    cy = instance;

    // Start particle animation
    stopAnimation = startAnimation(instance, containerEl, {
      isEnabled: () => flowStore.animationEnabled,
      animatedKinds: new Set(["calls", "data_flow"]),
    });
  }

  /** Highlight nodes connected to the selected node, dim others. */
  function highlightConnected(instance: cytoscape.Core, nodeId: string) {
    const node = instance.getElementById(nodeId);
    if (!node || node.empty()) return;

    const connected = node.closedNeighborhood();
    const connectedEdges = node.connectedEdges();

    instance.elements().addClass("dimmed").removeClass("highlighted").removeClass("selected-glow");
    connected.removeClass("dimmed").addClass("highlighted");
    connectedEdges.removeClass("dimmed").addClass("highlighted");
    node.addClass("selected-glow").removeClass("dimmed");
  }

  /** Clear all highlight classes. */
  function clearHighlights(instance: cytoscape.Core) {
    instance.elements().removeClass("dimmed highlighted selected-glow");
  }

  // Track which snapshot we've loaded roots for to avoid redundant reloads
  let loadedRootsKey = "";

  /** Fetch roots from API or compute client-side. */
  async function loadRoots() {
    if (!graph) return;
    const project = graphStore.selectedProject;
    const version = graphStore.selectedVersion;

    // Build a cache key — roots are per-snapshot, not per-focus
    const key = `${project ?? ""}:${version ?? ""}`;
    if (key === loadedRootsKey && flowStore.roots) return;
    loadedRootsKey = key;

    if (!project || !version) {
      flowStore.setRoots(computeClientRoots(graph));
      return;
    }

    flowStore.loading = true;
    flowStore.error = null;
    try {
      const roots = await api.getProjectRoots(project, version);
      flowStore.setRoots(roots);
    } catch {
      flowStore.setRoots(computeClientRoots(graph));
    } finally {
      flowStore.loading = false;
    }
  }

  // Load roots when snapshot changes (not on every graph focus change)
  $effect(() => {
    // Track project/version — these change when the snapshot changes
    const _project = graphStore.selectedProject;
    const _version = graphStore.selectedVersion;
    if (!graph) return;

    loadRoots();
  });

  // React to roots/expandedNodes changes — rebuild elements
  $effect(() => {
    if (!graph || !containerEl) return;
    const roots = flowStore.roots;
    const expanded = flowStore.expandedNodes;

    const elements = buildFlowElements(graph, roots, expanded);
    initCytoscape(elements);
  });

  // React to edge kind filter changes
  $effect(() => {
    if (!cy) return;
    const activeKinds = flowStore.activeEdgeKinds;

    cy.edges().forEach((edge) => {
      const kind = edge.data("kind") as string;
      if (activeKinds.has(kind)) {
        (edge as unknown as { show(): void }).show();
      } else {
        (edge as unknown as { hide(): void }).hide();
      }
    });
  });

  onMount(() => {
    return () => {
      stopAnimation?.();
      stopAnimation = null;
      cy?.destroy();
      cy = null;
      flowStore.reset();
    };
  });

  // Edge kinds for the toolbar toggle buttons
  const edgeKindButtons = [
    { kind: "calls", label: "Calls" },
    { kind: "depends", label: "Depends" },
    { kind: "data_flow", label: "DataFlow" },
    { kind: "implements", label: "Implements" },
    { kind: "extends", label: "Extends" },
    { kind: "exports", label: "Exports" },
  ];

  function getEdgeKindColor(kind: string): string {
    const style = EDGE_STYLES[kind];
    if (style) return getCssVar(style.cssVar) || getCssVar("--accent") || "#5b9bd5";
    return getCssVar("--accent") || "#5b9bd5";
  }
</script>

<div class="flow-container">
  <div class="flow-toolbar">
    <span class="toolbar-section">
      {#each edgeKindButtons as btn}
        <button
          class="edge-toggle"
          class:edge-toggle-active={flowStore.activeEdgeKinds.has(btn.kind)}
          onclick={() => flowStore.toggleEdgeKind(btn.kind)}
          aria-label="Toggle {btn.label} edges"
          title="Toggle {btn.label} edges"
        >
          <span
            class="edge-swatch"
            style="background: {getEdgeKindColor(btn.kind)};"
          ></span>
          {btn.label}
        </button>
      {/each}
    </span>
    <span class="toolbar-section">
      <button
        class="anim-toggle"
        class:anim-toggle-active={flowStore.animationEnabled}
        onclick={() => { flowStore.animationEnabled = !flowStore.animationEnabled; }}
        aria-label="Toggle animation"
      >
        {flowStore.animationEnabled ? "Anim On" : "Anim Off"}
      </button>
    </span>
  </div>

  <div class="flow-area">
    {#if !graph}
      <div class="center-message">
        <p>No graph data</p>
      </div>
    {:else}
      {#if flowStore.loading}
        <div class="center-message loading-overlay">
          <div class="spinner"></div>
          <p>Analyzing roots...</p>
        </div>
      {/if}
      <div class="cytoscape-container" bind:this={containerEl}></div>
    {/if}

    {#if flowStore.error}
      <div class="flow-error">{flowStore.error}</div>
    {/if}
  </div>

  {#if tooltip.visible}
    <div
      class="flow-tooltip"
      style="left: {tooltip.x + 12}px; top: {tooltip.y + 12}px;"
    >
      <div class="tooltip-name">{tooltip.label}</div>
      <div class="tooltip-row">
        <span class="tooltip-key">Kind</span>
        <span>{tooltip.kind}{tooltip.subKind ? ` / ${tooltip.subKind}` : ""}</span>
      </div>
      {#if tooltip.childCount > 0}
        <div class="tooltip-row">
          <span class="tooltip-key">Contains</span>
          <span>{tooltip.childCount} collapsed nodes</span>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .flow-container {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
    position: relative;
    overflow: hidden;
  }

  .flow-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.35rem 0.75rem;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .toolbar-section {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .edge-toggle {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text-muted);
    cursor: pointer;
    font-size: 0.75rem;
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
    transition: background 0.15s ease, color 0.15s ease, border-color 0.15s ease;
  }

  .edge-toggle:hover {
    background: var(--border);
    color: var(--text);
  }

  .edge-toggle-active {
    background: var(--bg);
    color: var(--text);
    border-color: var(--text-muted);
  }

  .edge-swatch {
    display: inline-block;
    width: 10px;
    height: 3px;
    border-radius: 1px;
  }

  .anim-toggle {
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text-muted);
    cursor: pointer;
    font-size: 0.75rem;
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
  }

  .anim-toggle-active {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .flow-area {
    flex: 1;
    position: relative;
    min-height: 0;
  }

  .cytoscape-container {
    width: 100%;
    height: 100%;
    position: absolute;
    top: 0;
    left: 0;
  }

  .center-message {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: 1rem;
    height: 100%;
  }

  .loading-overlay {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    z-index: 10;
    background: var(--bg, #1a1a2e);
    opacity: 0.85;
  }

  .spinner {
    width: 24px;
    height: 24px;
    border: 3px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    margin-bottom: 0.75rem;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .flow-error {
    position: absolute;
    bottom: 0.5rem;
    left: 0.5rem;
    background: var(--fail);
    color: #fff;
    padding: 0.3rem 0.6rem;
    border-radius: 4px;
    font-size: 0.8rem;
  }

  .flow-tooltip {
    position: fixed;
    z-index: 1000;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.5rem 0.75rem;
    font-size: 0.8rem;
    pointer-events: none;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    max-width: 280px;
  }

  .tooltip-name {
    font-weight: 600;
    margin-bottom: 0.2rem;
    color: var(--text);
  }

  .tooltip-row {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    color: var(--text);
    line-height: 1.5;
  }

  .tooltip-key {
    color: var(--text-muted);
  }
</style>
