<script lang="ts">
  import { lineRadial, curveBundle } from "d3-shape";
  import type { HierarchyPointNode } from "d3-hierarchy";
  import { zoom as d3zoom, zoomIdentity } from "d3-zoom";
  import { select } from "d3-selection";
  import { onMount } from "svelte";
  import type { CytoscapeGraph } from "../lib/types";
  import { buildHierarchy, type TreeNode } from "../lib/hierarchy";
  import {
    computeBundledEdges,
    createRadialCluster,
    type BundledEdge,
  } from "../lib/edge-bundling";
  import { EDGE_STYLES, KIND_COLORS } from "../lib/visual-encoding";
  interface Props {
    graph: CytoscapeGraph | null;
    onselectnode?: (nodeId: string) => void;
  }

  let { graph, onselectnode }: Props = $props();

  let containerWidth = $state(800);
  let containerHeight = $state(600);
  let tension = $state(0.85);
  let svgEl = $state<SVGSVGElement>(undefined!);


  // Hover state
  let hoveredNodeId = $state<string | null>(null);

  // Tooltip state
  let tooltip = $state<{
    visible: boolean;
    x: number;
    y: number;
    label: string;
    kind: string;
    subKind: string;
  }>({ visible: false, x: 0, y: 0, label: "", kind: "", subKind: "" });

  // Zoom transform
  let transformStr = $state("translate(0,0) scale(1)");

  function getCssVar(name: string): string {
    return getComputedStyle(document.documentElement)
      .getPropertyValue(name)
      .trim();
  }

  // Build hierarchy from graph
  let fullHierarchy = $derived.by(() => {
    if (!graph) return null;
    return buildHierarchy(graph);
  });

  // The layout root is the full hierarchy (focus/subtree filtering handled by App.svelte)
  let drillRoot = $derived(fullHierarchy as unknown as HierarchyPointNode<TreeNode> | null);

  // Layout dimensions
  let radius = $derived(Math.min(containerWidth, containerHeight) / 2 - 80);
  let innerRadius = $derived(Math.max(radius - 40, 60));

  // Compute radial cluster layout
  let clusterRoot = $derived.by((): HierarchyPointNode<TreeNode> | null => {
    if (!drillRoot || innerRadius <= 0) return null;
    return createRadialCluster(
      drillRoot.copy() as unknown as HierarchyPointNode<TreeNode>,
      innerRadius,
    );
  });

  // Compute bundled edges
  let bundledEdges = $derived.by((): BundledEdge[] => {
    if (!clusterRoot || !graph) return [];
    return computeBundledEdges(clusterRoot, graph.elements.edges);
  });

  // Get leaf nodes for rendering
  let leafNodes = $derived.by(
    (): Array<{
      id: string;
      label: string;
      kind: string;
      subKind: string;
      x: number;
      y: number;
      angle: number;
      hasChildren: boolean;
    }> => {
      if (!clusterRoot) return [];
      const nodes: Array<{
        id: string;
        label: string;
        kind: string;
        subKind: string;
        x: number;
        y: number;
        angle: number;
        hasChildren: boolean;
      }> = [];
      // Collect direct children (one level deeper than drill root)
      clusterRoot.each((node) => {
        if (node === clusterRoot) return;
        if (node.children && node.children.length > 0 && node.depth === 1)
          return; // Skip intermediate parents at depth 1 from drill root
        const angleRad = (node.x * Math.PI) / 180;
        const x = Math.sin(angleRad) * node.y;
        const y = -Math.cos(angleRad) * node.y;
        nodes.push({
          id: node.data.id,
          label: node.data.label,
          kind: node.data.kind,
          subKind: node.data.sub_kind,
          x,
          y,
          angle: node.x,
          hasChildren: (node.children?.length ?? 0) > 0,
        });
      });
      return nodes;
    },
  );

  // Build edges-by-node lookup for hover highlighting
  let edgesByNode = $derived.by(() => {
    const incoming = new Map<string, BundledEdge[]>();
    const outgoing = new Map<string, BundledEdge[]>();
    for (const edge of bundledEdges) {
      if (!outgoing.has(edge.sourceId)) outgoing.set(edge.sourceId, []);
      outgoing.get(edge.sourceId)!.push(edge);
      if (!incoming.has(edge.targetId)) incoming.set(edge.targetId, []);
      incoming.get(edge.targetId)!.push(edge);
    }
    return { incoming, outgoing };
  });

  // Set of connected node IDs when hovering
  let connectedNodeIds = $derived.by(() => {
    if (!hoveredNodeId) return new Set<string>();
    const ids = new Set<string>();
    ids.add(hoveredNodeId);
    const inc = edgesByNode.incoming.get(hoveredNodeId) ?? [];
    const out = edgesByNode.outgoing.get(hoveredNodeId) ?? [];
    for (const e of inc) ids.add(e.sourceId);
    for (const e of out) ids.add(e.targetId);
    return ids;
  });

  // Line generator for bundled edges
  let lineGen = $derived.by(() => {
    return lineRadial<[number, number]>()
      .angle((d) => d[0])
      .radius((d) => d[1])
      .curve(curveBundle.beta(tension));
  });


  // Set up d3-zoom
  onMount(() => {
    if (!svgEl) return;

    const zoomBehavior = d3zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.2, 5])
      .on("zoom", (event) => {
        const t = event.transform;
        transformStr = `translate(${t.x},${t.y}) scale(${t.k})`;
      });

    select(svgEl).call(zoomBehavior);

    // Reset zoom on double-click
    select(svgEl).on("dblclick.zoom", () => {
      select(svgEl)
        .transition()
        .duration(300)
        .call(zoomBehavior.transform, zoomIdentity);
    });

    return () => {
      select(svgEl).on(".zoom", null);
    };
  });

  function getEdgeColor(kind: string): string {
    const style = EDGE_STYLES[kind];
    if (style) return getCssVar(style.cssVar) || getCssVar("--accent") || "#5b9bd5";
    return getCssVar("--accent") || "#5b9bd5";
  }

  function getNodeColor(kind: string): string {
    const cssVar = KIND_COLORS[kind];
    if (cssVar) return getCssVar(cssVar) || getCssVar("--accent") || "#5b9bd5";
    return getCssVar("--accent") || "#5b9bd5";
  }

  function getEdgeOpacity(edge: BundledEdge): number {
    if (!hoveredNodeId) return 0.4;
    if (edge.sourceId === hoveredNodeId || edge.targetId === hoveredNodeId)
      return 0.85;
    return 0.04;
  }

  function getHighlightColor(edge: BundledEdge): string {
    if (!hoveredNodeId) return getEdgeColor(edge.kind);
    if (edge.sourceId === hoveredNodeId)
      return getCssVar("--fail") || "#f44336"; // outgoing = red
    if (edge.targetId === hoveredNodeId)
      return getCssVar("--pass") || "#4caf50"; // incoming = green
    return getEdgeColor(edge.kind);
  }

  function handleNodeClick(nodeId: string) {
    onselectnode?.(nodeId);
  }

  function handleNodeEnter(
    event: MouseEvent,
    node: { id: string; label: string; kind: string; subKind: string },
  ) {
    hoveredNodeId = node.id;
    tooltip = {
      visible: true,
      x: event.clientX,
      y: event.clientY,
      label: node.label,
      kind: node.kind,
      subKind: node.subKind,
    };
  }

  function handleNodeMove(event: MouseEvent) {
    if (tooltip.visible) {
      tooltip.x = event.clientX;
      tooltip.y = event.clientY;
    }
  }

  function handleNodeLeave() {
    hoveredNodeId = null;
    tooltip = { ...tooltip, visible: false };
  }

  /** Label position: outside the circle, rotated for readability. */
  function labelTransform(angle: number): string {
    const labelRadius = innerRadius + 12;
    const angleRad = (angle * Math.PI) / 180;
    const x = Math.sin(angleRad) * labelRadius;
    const y = -Math.cos(angleRad) * labelRadius;
    // Rotate text to follow the circle
    const rotate = angle > 180 ? angle - 270 : angle - 90;
    return `translate(${x},${y}) rotate(${rotate})`;
  }

  function labelAnchor(angle: number): string {
    return angle > 180 ? "end" : "start";
  }
</script>

<div
  class="bundle-container"
  bind:clientWidth={containerWidth}
  bind:clientHeight={containerHeight}
>
  <div class="bundle-toolbar">
    <label class="tension-label">
      Tension:
      <input
        type="range"
        min="0"
        max="1"
        step="0.05"
        bind:value={tension}
        class="tension-slider"
      />
      <span class="tension-value">{tension.toFixed(2)}</span>
    </label>
  </div>

  <div class="bundle-area">
    {#if leafNodes.length === 0 && graph}
      <div class="center-message">
        <p>No nodes to display</p>
      </div>
    {:else if leafNodes.length > 0}
      <svg
        bind:this={svgEl}
        viewBox="{-containerWidth / 2} {-containerHeight / 2} {containerWidth} {containerHeight}"
        class="bundle-svg"
      >
        <g transform={transformStr}>
          <!-- Bundled edge paths -->
          {#each bundledEdges as edge}
            <path
              d={lineGen(edge.points) ?? ""}
              fill="none"
              stroke={getHighlightColor(edge)}
              stroke-width={hoveredNodeId && (edge.sourceId === hoveredNodeId || edge.targetId === hoveredNodeId) ? 2 : 1.5}
              stroke-opacity={getEdgeOpacity(edge)}
              class="bundle-edge"
            />
          {/each}

          <!-- Node dots -->
          {#each leafNodes as node (node.id)}
            <circle
              cx={node.x}
              cy={node.y}
              r={node.hasChildren ? 5 : 3.5}
              fill={getNodeColor(node.kind)}
              class="bundle-node"
              class:bundle-node-faded={hoveredNodeId !== null && !connectedNodeIds.has(node.id)}
              class:bundle-node-parent={node.hasChildren}
              role="button"
              tabindex="0"
              aria-label="{node.label} ({node.kind})"
              onclick={() => handleNodeClick(node.id)}
              onkeydown={(e) => {
                if (e.key === "Enter" || e.key === " ")
                  handleNodeClick(node.id);
              }}
              onmouseenter={(e) => handleNodeEnter(e, node)}
              onmousemove={handleNodeMove}
              onmouseleave={handleNodeLeave}
            />
          {/each}

          <!-- Labels -->
          {#each leafNodes as node (node.id)}
            <text
              transform={labelTransform(node.angle)}
              text-anchor={labelAnchor(node.angle)}
              dominant-baseline="central"
              class="bundle-label"
              class:bundle-label-highlighted={hoveredNodeId !== null && connectedNodeIds.has(node.id)}
              class:bundle-label-faded={hoveredNodeId !== null && !connectedNodeIds.has(node.id)}
            >
              {node.label}
            </text>
          {/each}
        </g>
      </svg>
    {/if}
  </div>

  {#if tooltip.visible}
    <div
      class="bundle-tooltip"
      style="left: {tooltip.x + 12}px; top: {tooltip.y + 12}px;"
    >
      <div class="tooltip-name">{tooltip.label}</div>
      <div class="tooltip-row">
        <span class="tooltip-key">Kind</span>
        <span>{tooltip.kind}{tooltip.subKind ? ` / ${tooltip.subKind}` : ""}</span>
      </div>
    </div>
  {/if}
</div>

<style>
  .bundle-container {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
    position: relative;
    overflow: hidden;
  }

  .bundle-toolbar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.75rem;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .tension-label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  .tension-slider {
    width: 120px;
    accent-color: var(--accent);
  }

  .tension-value {
    min-width: 2rem;
    text-align: center;
    font-variant-numeric: tabular-nums;
    font-size: 0.8rem;
  }

  .bundle-area {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 0;
  }

  .bundle-svg {
    width: 100%;
    height: 100%;
    cursor: grab;
  }

  .bundle-svg:active {
    cursor: grabbing;
  }

  .bundle-edge {
    pointer-events: none;
    transition: stroke-opacity 0.15s ease;
  }

  .bundle-node {
    cursor: pointer;
    stroke: var(--surface);
    stroke-width: 1;
    transition: opacity 0.15s ease;
  }

  .bundle-node-faded {
    opacity: 0.2;
  }

  .bundle-node-parent {
    stroke-width: 2;
    stroke: var(--bg);
  }

  .bundle-node:hover {
    stroke: var(--text);
    stroke-width: 2;
  }

  .bundle-node:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .bundle-label {
    font-size: 9px;
    fill: var(--text);
    pointer-events: none;
    transition: opacity 0.15s ease, font-weight 0.15s ease;
  }

  .bundle-label-highlighted {
    font-weight: bold;
    fill: var(--text);
  }

  .bundle-label-faded {
    opacity: 0.15;
  }

  .center-message {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: 1rem;
    height: 100%;
  }

  .bundle-tooltip {
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
