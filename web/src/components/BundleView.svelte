<script lang="ts">
  import { lineRadial, curveBundle } from "d3-shape";
  import { scaleSqrt, scaleLog } from "d3-scale";
  import type { HierarchyPointNode } from "d3-hierarchy";
  import { zoom as d3zoom, zoomIdentity } from "d3-zoom";
  import { select } from "d3-selection";
  import { onMount } from "svelte";
  import type { CytoscapeGraph } from "../lib/types";
  import { buildHierarchy, type TreeNode } from "../lib/hierarchy";
  import {
    computeBundledEdges,
    computeArcEdges,
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
    descendants: number;
    incomingCount: number;
    outgoingCount: number;
  }>({ visible: false, x: 0, y: 0, label: "", kind: "", subKind: "", descendants: 0, incomingCount: 0, outgoingCount: 0 });

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

  // Layout dimensions — more room for labels
  let radius = $derived(Math.min(containerWidth, containerHeight) / 2 - 120);
  let innerRadius = $derived(Math.max(radius - 50, 60));

  // Compute radial cluster layout
  let clusterRoot = $derived.by((): HierarchyPointNode<TreeNode> | null => {
    if (!drillRoot || innerRadius <= 0) return null;
    return createRadialCluster(
      drillRoot.copy() as unknown as HierarchyPointNode<TreeNode>,
      innerRadius,
    );
  });

  // Detect flat tree (root with only leaf children → use arcs instead of hierarchy bundling)
  let isFlat = $derived.by(() => {
    if (!clusterRoot) return false;
    return clusterRoot.height <= 1;
  });

  // Compute bundled edges — adaptive: arcs for flat trees, hierarchy bundling for deeper
  let bundledEdges = $derived.by((): BundledEdge[] => {
    if (!clusterRoot || !graph) return [];
    if (isFlat) {
      return computeArcEdges(clusterRoot, graph.elements.edges);
    }
    return computeBundledEdges(clusterRoot, graph.elements.edges);
  });

  // Build _childCount lookup from graph node data
  let childCountMap = $derived.by(() => {
    const map = new Map<string, number>();
    if (!graph) return map;
    for (const node of graph.elements.nodes) {
      const d = node.data as unknown as Record<string, unknown>;
      if (typeof d._childCount === "number") {
        map.set(node.data.id, d._childCount);
      }
    }
    return map;
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
      childCount: number;
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
        childCount: number;
      }> = [];
      // Collect only leaf nodes (nodes without children in the layout tree)
      clusterRoot.each((node) => {
        if (node === clusterRoot) return;
        if (node.children && node.children.length > 0) return;
        const angleRad = (node.x * Math.PI) / 180;
        const x = Math.sin(angleRad) * node.y;
        const y = -Math.cos(angleRad) * node.y;
        const cc = childCountMap.get(node.data.id)
          ?? (node.data.metadata as Record<string, unknown> | undefined)?._childCount as number | undefined
          ?? 0;
        nodes.push({
          id: node.data.id,
          label: node.data.label,
          kind: node.data.kind,
          subKind: node.data.sub_kind,
          x,
          y,
          angle: node.x,
          childCount: cc,
        });
      });
      return nodes;
    },
  );

  // d3 scales for visual encoding
  let radiusScale = $derived.by(() => {
    const counts = leafNodes.map((n) => n.childCount).filter((c) => c > 0);
    const maxCount = counts.length > 0 ? Math.max(...counts) : 1;
    return scaleSqrt().domain([0, maxCount]).range([6, 28]);
  });

  let edgeWidthScale = $derived.by(() => {
    const edgeCounts = bundledEdges.map((e) => e.count).filter((c) => c > 0);
    const maxCount = edgeCounts.length > 0 ? Math.max(...edgeCounts) : 1;
    return scaleLog().domain([1, Math.max(maxCount, 2)]).range([1.5, 10]).clamp(true);
  });

  // Whether to always show labels (≤60 nodes) or hover-only
  let alwaysShowLabels = $derived(leafNodes.length <= 60);

  // Build edges-by-node lookup for hover highlighting
  let edgesByNode = $derived.by(() => {
    const incoming = new Map<string, BundledEdge[]>();
    const outgoing = new Map<string, BundledEdge[]>();

    function addIncoming(nodeId: string, edge: BundledEdge) {
      if (!incoming.has(nodeId)) incoming.set(nodeId, []);
      incoming.get(nodeId)!.push(edge);
    }
    function addOutgoing(nodeId: string, edge: BundledEdge) {
      if (!outgoing.has(nodeId)) outgoing.set(nodeId, []);
      outgoing.get(nodeId)!.push(edge);
    }

    for (const edge of bundledEdges) {
      addOutgoing(edge.sourceId, edge);
      addIncoming(edge.targetId, edge);
    }
    return { incoming, outgoing };
  });

  // Set of connected leaf node IDs when hovering
  let connectedNodeIds = $derived.by(() => {
    if (!hoveredNodeId) return new Set<string>();
    const ids = new Set<string>();
    ids.add(hoveredNodeId);
    const inc = edgesByNode.incoming.get(hoveredNodeId) ?? [];
    const out = edgesByNode.outgoing.get(hoveredNodeId) ?? [];
    for (const e of inc) {
      ids.add(e.sourceId);
    }
    for (const e of out) {
      ids.add(e.targetId);
    }
    return ids;
  });

  // Line generator for bundled edges
  let lineGen = $derived.by(() => {
    return lineRadial<[number, number]>()
      .angle((d) => d[0])
      .radius((d) => d[1])
      .curve(curveBundle.beta(tension));
  });

  // Legend entries derived from the actual edge kinds present
  let legendEntries = $derived.by(() => {
    const kindCounts = new Map<string, number>();
    for (const edge of bundledEdges) {
      kindCounts.set(edge.kind, (kindCounts.get(edge.kind) ?? 0) + 1);
    }
    return [...kindCounts.entries()]
      .sort((a, b) => b[1] - a[1])
      .map(([kind, count]) => ({ kind, count, color: getEdgeColor(kind) }));
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

  function getNodeRadius(childCount: number): number {
    return childCount > 0 ? radiusScale(childCount) : 6;
  }

  function getEdgeWidth(edge: BundledEdge): number {
    return edgeWidthScale(Math.max(edge.count, 1));
  }

  /** Check whether the hovered leaf is the edge's source or target. */
  function isEdgeConnected(edge: BundledEdge): "source" | "target" | false {
    if (!hoveredNodeId) return false;
    const out = edgesByNode.outgoing.get(hoveredNodeId);
    if (out?.includes(edge)) return "source";
    const inc = edgesByNode.incoming.get(hoveredNodeId);
    if (inc?.includes(edge)) return "target";
    return false;
  }

  function getEdgeOpacity(edge: BundledEdge): number {
    if (!hoveredNodeId) return 0.4;
    if (isEdgeConnected(edge)) return 0.85;
    return 0.04;
  }

  function getHighlightColor(edge: BundledEdge): string {
    if (!hoveredNodeId) return getEdgeColor(edge.kind);
    const conn = isEdgeConnected(edge);
    if (conn === "source")
      return getCssVar("--fail") || "#f44336"; // outgoing = red
    if (conn === "target")
      return getCssVar("--pass") || "#4caf50"; // incoming = green
    return getEdgeColor(edge.kind);
  }

  function getEdgeStrokeWidth(edge: BundledEdge): number {
    if (hoveredNodeId && isEdgeConnected(edge)) {
      return Math.max(getEdgeWidth(edge), 2.5);
    }
    return getEdgeWidth(edge);
  }

  function handleNodeClick(nodeId: string) {
    onselectnode?.(nodeId);
  }

  function handleNodeEnter(
    event: MouseEvent,
    node: { id: string; label: string; kind: string; subKind: string; childCount: number },
  ) {
    hoveredNodeId = node.id;
    const inc = edgesByNode.incoming.get(node.id) ?? [];
    const out = edgesByNode.outgoing.get(node.id) ?? [];
    tooltip = {
      visible: true,
      x: event.clientX,
      y: event.clientY,
      label: node.label,
      kind: node.kind,
      subKind: node.subKind,
      descendants: node.childCount,
      incomingCount: inc.reduce((s, e) => s + e.count, 0),
      outgoingCount: out.reduce((s, e) => s + e.count, 0),
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
  function labelTransform(angle: number, childCount: number): string {
    const nodeR = getNodeRadius(childCount);
    const labelRadius = innerRadius + nodeR + 4;
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
              stroke-width={getEdgeStrokeWidth(edge)}
              stroke-opacity={getEdgeOpacity(edge)}
              class="bundle-edge"
            />
          {/each}

          <!-- Node dots with size encoding -->
          {#each leafNodes as node (node.id)}
            <circle
              cx={node.x}
              cy={node.y}
              r={getNodeRadius(node.childCount)}
              fill={getNodeColor(node.kind)}
              class="bundle-node"
              class:bundle-node-faded={hoveredNodeId !== null && !connectedNodeIds.has(node.id)}
              role="button"
              tabindex="0"
              aria-label="{node.label} ({node.kind}{node.childCount > 0 ? `, ${node.childCount} descendants` : ''})"
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
          {#if alwaysShowLabels}
            <!-- Always-visible labels when ≤60 nodes -->
            {#each leafNodes as node (node.id)}
              <text
                transform={labelTransform(node.angle, node.childCount)}
                text-anchor={labelAnchor(node.angle)}
                dominant-baseline="central"
                class="bundle-label"
                class:bundle-label-highlighted={hoveredNodeId !== null && connectedNodeIds.has(node.id)}
                class:bundle-label-faded={hoveredNodeId !== null && !connectedNodeIds.has(node.id)}
              >
                {node.label}
              </text>
            {/each}
          {:else if hoveredNodeId}
            <!-- Hover-only labels when >60 nodes -->
            {#each leafNodes as node (node.id)}
              {#if connectedNodeIds.has(node.id)}
                <text
                  transform={labelTransform(node.angle, node.childCount)}
                  text-anchor={labelAnchor(node.angle)}
                  dominant-baseline="central"
                  class="bundle-label bundle-label-highlighted"
                >
                  {node.label}
                </text>
              {/if}
            {/each}
          {/if}
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
      {#if tooltip.descendants > 0}
        <div class="tooltip-row">
          <span class="tooltip-key">Descendants</span>
          <span>{tooltip.descendants}</span>
        </div>
      {/if}
      {#if tooltip.incomingCount > 0 || tooltip.outgoingCount > 0}
        <div class="tooltip-row">
          <span class="tooltip-key tooltip-incoming">Incoming</span>
          <span>{tooltip.incomingCount}</span>
        </div>
        <div class="tooltip-row">
          <span class="tooltip-key tooltip-outgoing">Outgoing</span>
          <span>{tooltip.outgoingCount}</span>
        </div>
      {/if}
    </div>
  {/if}

  {#if legendEntries.length > 0}
    <div class="bundle-legend">
      {#each legendEntries as entry}
        <span class="legend-item">
          <span class="legend-swatch" style="background: {entry.color};"></span>
          <span class="legend-label">{entry.kind}</span>
          <span class="legend-count">{entry.count}</span>
        </span>
      {/each}
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
    stroke-width: 1.5;
    transition: opacity 0.15s ease;
  }

  .bundle-node-faded {
    opacity: 0.2;
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
    font-size: 11px;
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

  .tooltip-incoming {
    color: var(--pass, #4caf50);
  }

  .tooltip-outgoing {
    color: var(--fail, #f44336);
  }

  .bundle-legend {
    position: absolute;
    bottom: 0.75rem;
    right: 0.75rem;
    display: flex;
    gap: 0.75rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.3rem 0.6rem;
    font-size: 0.75rem;
    pointer-events: none;
  }

  .legend-item {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    color: var(--text);
  }

  .legend-swatch {
    display: inline-block;
    width: 12px;
    height: 3px;
    border-radius: 1px;
  }

  .legend-label {
    text-transform: capitalize;
  }

  .legend-count {
    color: var(--text-muted);
  }
</style>
