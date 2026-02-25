<script lang="ts">
  import { partition } from "d3-hierarchy";
  import type { HierarchyNode, HierarchyRectangularNode } from "d3-hierarchy";
  import { arc as d3arc } from "d3-shape";
  import { scaleOrdinal, scaleSequential } from "d3-scale";
  import { schemeTableau10, interpolateRdYlGn, interpolateBlues } from "d3-scale-chromatic";
  import type { CytoscapeGraph } from "../lib/types";
  import { buildHierarchy, sumByMetric, getMetric, type TreeNode } from "../lib/hierarchy";
  type ColourMode = "language" | "kind" | "depth" | "fan-out";

  interface Props {
    graph: CytoscapeGraph | null;
    onselectnode?: (nodeId: string) => void;
  }

  let { graph, onselectnode }: Props = $props();

  let containerWidth = $state(800);
  let containerHeight = $state(600);
  let colourMode = $state<ColourMode>("language");


  // Tooltip state
  let tooltip = $state<{
    visible: boolean;
    x: number;
    y: number;
    node: TreeNode | null;
    value: number;
  }>({ visible: false, x: 0, y: 0, node: null, value: 0 });

  // Maximum visible rings from the current root
  const MAX_VISIBLE_DEPTH = 3;

  // Build the full hierarchy from graph data
  let fullHierarchy = $derived.by((): HierarchyNode<TreeNode> | null => {
    if (!graph) return null;
    return buildHierarchy(graph);
  });

  // The layout root is the full hierarchy (focus/subtree filtering handled by App.svelte)
  let drillRoot = $derived(fullHierarchy);

  // Compute the maximum fan-out across all leaves for colour scaling
  let maxFanOut = $derived.by((): number => {
    if (!fullHierarchy) return 1;
    let max = 1;
    fullHierarchy.each((node) => {
      const fo = getMetric(node.data, "fan_out");
      if (fo > max) max = fo;
    });
    return max;
  });

  // Compute the maximum depth for the depth colour mode
  let maxDepth = $derived.by((): number => {
    if (!fullHierarchy) return 1;
    let max = 0;
    fullHierarchy.each((node) => {
      if (node.depth > max) max = node.depth;
    });
    return Math.max(max, 1);
  });

  // Collect unique languages for categorical colour mapping
  let languageSet = $derived.by((): string[] => {
    if (!fullHierarchy) return [];
    const langs = new Set<string>();
    fullHierarchy.each((node) => {
      if (node.data.language) langs.add(node.data.language);
    });
    return Array.from(langs).sort();
  });

  // Colour scales
  let languageScale = $derived(
    scaleOrdinal<string, string>().domain(languageSet).range([...schemeTableau10]),
  );

  let kindScale = $derived(
    scaleOrdinal<string, string>()
      .domain(["system", "service", "component", "unit"])
      .range(["#4e79a7", "#f28e2b", "#59a14f", "#e15759"]),
  );

  let depthScale = $derived(
    scaleSequential(interpolateBlues).domain([0, maxDepth]),
  );

  let fanOutScale = $derived(
    scaleSequential((t: number) => interpolateRdYlGn(1 - t)).domain([0, maxFanOut]),
  );

  // Compute the partition layout
  let radius = $derived(Math.min(containerWidth, containerHeight) / 2 - 20);

  let arcs = $derived.by((): HierarchyRectangularNode<TreeNode>[] => {
    if (!drillRoot || radius <= 0) return [];

    const root = sumByMetric(drillRoot.copy(), "loc");

    const layout = partition<TreeNode>().size([2 * Math.PI, radius]);

    const laid = layout(root);

    // Collect visible arcs: skip root (it becomes the centre), limit depth
    const result: HierarchyRectangularNode<TreeNode>[] = [];
    laid.each((node) => {
      if (node.depth > 0 && node.depth <= MAX_VISIBLE_DEPTH) {
        result.push(node);
      }
    });
    return result;
  });

  // The centre node (current drill root)
  let centreNode = $derived.by((): TreeNode | null => {
    return drillRoot?.data ?? null;
  });

  // Arc generator — x0/x1 already in radians after partition with size [2*PI, radius]
  // y0/y1 are radius values
  let arcGen = $derived.by(() => {
    const gen = d3arc<HierarchyRectangularNode<TreeNode>>()
      .startAngle((d) => d.x0)
      .endAngle((d) => d.x1)
      .innerRadius((d) => d.y0)
      .outerRadius((d) => d.y1)
      .padAngle(0.002)
      .padRadius(radius);
    return gen;
  });


  function getColor(node: HierarchyRectangularNode<TreeNode>): string {
    switch (colourMode) {
      case "language":
        return node.data.language ? languageScale(node.data.language) : "#888";
      case "kind":
        return kindScale(node.data.kind);
      case "depth":
        return depthScale(node.depth);
      case "fan-out":
        return fanOutScale(getMetric(node.data, "fan_out"));
    }
  }

  function handleArcClick(node: HierarchyRectangularNode<TreeNode>) {
    onselectnode?.(node.data.id);
  }

  function handleMouseEnter(
    event: MouseEvent,
    node: HierarchyRectangularNode<TreeNode>,
  ) {
    tooltip = {
      visible: true,
      x: event.clientX,
      y: event.clientY,
      node: node.data,
      value: node.value ?? 0,
    };
  }

  function handleMouseMove(event: MouseEvent) {
    if (tooltip.visible) {
      tooltip.x = event.clientX;
      tooltip.y = event.clientY;
    }
  }

  function handleMouseLeave() {
    tooltip = { ...tooltip, visible: false };
  }

  /** Check if an arc is wide enough to display a label. */
  function shouldShowLabel(node: HierarchyRectangularNode<TreeNode>): boolean {
    const angleSpan = node.x1 - node.x0;
    // Need enough angular span and radial depth for readable text
    return angleSpan > 0.08 && (node.y1 - node.y0) > 20;
  }

  /** Compute the transform for a label at an arc's centroid. */
  function labelTransform(node: HierarchyRectangularNode<TreeNode>): string {
    const midAngle = (node.x0 + node.x1) / 2;
    const midRadius = (node.y0 + node.y1) / 2;
    const x = Math.sin(midAngle) * midRadius;
    const y = -Math.cos(midAngle) * midRadius;
    // Convert to degrees and rotate text along the arc
    const degrees = (midAngle * 180) / Math.PI;
    // Flip text on the bottom half so it reads left-to-right
    const rotate = degrees > 180 ? degrees - 270 : degrees - 90;
    return `translate(${x},${y}) rotate(${rotate})`;
  }

  /** Truncate a label to fit the available arc length. */
  function truncateLabel(label: string, node: HierarchyRectangularNode<TreeNode>): string {
    const angleSpan = node.x1 - node.x0;
    const midRadius = (node.y0 + node.y1) / 2;
    const arcLength = angleSpan * midRadius;
    // Rough estimate: ~6px per character at 10px font size
    const maxChars = Math.floor(arcLength / 6);
    if (maxChars <= 0) return "";
    if (label.length <= maxChars) return label;
    return label.slice(0, maxChars - 1) + "\u2026";
  }
</script>

<div
  class="sunburst-container"
  bind:clientWidth={containerWidth}
  bind:clientHeight={containerHeight}
>
  <div class="sunburst-toolbar">
    <span class="toolbar-label">Colour:</span>
    {#each [
      { value: "language" as ColourMode, label: "Language" },
      { value: "kind" as ColourMode, label: "Kind" },
      { value: "depth" as ColourMode, label: "Depth" },
      { value: "fan-out" as ColourMode, label: "Fan-out" },
    ] as item}
      <button
        class="colour-btn"
        class:colour-btn-active={colourMode === item.value}
        onclick={() => { colourMode = item.value; }}
      >{item.label}</button>
    {/each}
  </div>

  <div class="sunburst-area">
    {#if arcs.length === 0 && graph}
      <div class="center-message">
        <p>No nodes to display in sunburst</p>
      </div>
    {:else if arcs.length > 0}
      <svg
        viewBox="{-containerWidth / 2} {-containerHeight / 2} {containerWidth} {containerHeight}"
        class="sunburst-svg"
      >
        <!-- Centre circle -->
        <circle
          cx="0"
          cy="0"
          r={arcs.length > 0 ? arcs[0].y0 : 30}
          class="sunburst-centre"
          aria-label={centreNode?.label ?? "Root"}
        />
        <!-- Centre label -->
        <text
          x="0"
          y="0"
          text-anchor="middle"
          dominant-baseline="central"
          class="sunburst-centre-label"
        >
          {centreNode?.label ?? ""}
        </text>

        <!-- Arc segments -->
        {#each arcs as node (node.data.id)}
          <path
            d={arcGen(node) ?? ""}
            fill={getColor(node)}
            class="sunburst-arc"
            role="button"
            tabindex="0"
            aria-label="{node.data.label} ({node.data.kind})"
            onclick={() => handleArcClick(node)}
            onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") handleArcClick(node); }}
            onmouseenter={(e) => handleMouseEnter(e, node)}
            onmousemove={handleMouseMove}
            onmouseleave={handleMouseLeave}
          />
        {/each}

        <!-- Arc labels -->
        {#each arcs as node (node.data.id)}
          {#if shouldShowLabel(node)}
            <text
              transform={labelTransform(node)}
              text-anchor="middle"
              dominant-baseline="central"
              class="sunburst-label"
            >
              {truncateLabel(node.data.label, node)}
            </text>
          {/if}
        {/each}
      </svg>
    {/if}
  </div>

  {#if arcs.length > 0}
    <div class="sunburst-legend">
      {#if colourMode === "language"}
        {#each languageSet as lang}
          <div class="legend-item">
            <span class="legend-swatch" style="background-color: {languageScale(lang)}"></span>
            <span class="legend-name">{lang}</span>
          </div>
        {/each}
        {#if languageSet.length === 0}
          <span class="legend-note">No languages</span>
        {/if}
      {:else if colourMode === "kind"}
        {#each ["system", "service", "component", "unit"] as k}
          <div class="legend-item">
            <span class="legend-swatch" style="background-color: {kindScale(k)}"></span>
            <span class="legend-name">{k}</span>
          </div>
        {/each}
      {:else if colourMode === "depth"}
        <div class="legend-gradient-row">
          <span class="legend-gradient-label">Shallow</span>
          <div class="legend-gradient depth-gradient"></div>
          <span class="legend-gradient-label">Deep</span>
        </div>
      {:else if colourMode === "fan-out"}
        <div class="legend-gradient-row">
          <span class="legend-gradient-label">Low</span>
          <div class="legend-gradient fanout-gradient"></div>
          <span class="legend-gradient-label">High</span>
        </div>
      {/if}
    </div>
  {/if}

  {#if tooltip.visible && tooltip.node}
    <div
      class="sunburst-tooltip"
      style="left: {tooltip.x + 12}px; top: {tooltip.y + 12}px;"
    >
      <div class="tooltip-name">{tooltip.node.label}</div>
      <div class="tooltip-row">
        <span class="tooltip-key">Kind</span>
        <span>{tooltip.node.kind}{tooltip.node.sub_kind ? ` / ${tooltip.node.sub_kind}` : ""}</span>
      </div>
      <div class="tooltip-row">
        <span class="tooltip-key">LOC</span>
        <span>{tooltip.value.toLocaleString()}</span>
      </div>
      <div class="tooltip-row">
        <span class="tooltip-key">Fan-in</span>
        <span>{getMetric(tooltip.node, "fan_in")}</span>
      </div>
      <div class="tooltip-row">
        <span class="tooltip-key">Fan-out</span>
        <span>{getMetric(tooltip.node, "fan_out")}</span>
      </div>
      {#if getMetric(tooltip.node, "_childCount") > 0}
        <div class="tooltip-row">
          <span class="tooltip-key">Contains</span>
          <span>{getMetric(tooltip.node, "_childCount")} collapsed nodes</span>
        </div>
      {/if}
      {#if tooltip.node.language}
        <div class="tooltip-row">
          <span class="tooltip-key">Language</span>
          <span>{tooltip.node.language}</span>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .sunburst-container {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
    position: relative;
    overflow: hidden;
  }

  .sunburst-toolbar {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.4rem 0.75rem;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .toolbar-label {
    font-size: 0.8rem;
    color: var(--text-muted);
    margin-right: 0.25rem;
  }

  .colour-btn {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    font-size: 0.75rem;
    padding: 0.2rem 0.5rem;
    border-radius: 3px;
    cursor: pointer;
  }

  .colour-btn-active {
    background: var(--accent);
    color: #fff;
    border-color: var(--accent);
  }

  .sunburst-area {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 0;
  }

  .sunburst-svg {
    width: 100%;
    height: 100%;
  }

  .sunburst-centre {
    fill: var(--surface);
    stroke: var(--border);
    stroke-width: 1;
  }

  .sunburst-centre-label {
    font-size: 12px;
    fill: var(--text);
    pointer-events: none;
    font-weight: 600;
  }

  .sunburst-arc {
    stroke: var(--surface);
    stroke-width: 1;
    cursor: pointer;
    transition: opacity 0.15s ease;
  }

  .sunburst-arc:hover {
    opacity: 0.8;
  }

  .sunburst-arc:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .sunburst-label {
    font-size: 10px;
    fill: #fff;
    pointer-events: none;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.7);
  }

  .sunburst-legend {
    position: absolute;
    bottom: 8px;
    right: 8px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.4rem 0.5rem;
    font-size: 0.7rem;
    max-height: 200px;
    overflow-y: auto;
    opacity: 0.9;
    z-index: 5;
  }

  .legend-item {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.1rem 0;
  }

  .legend-swatch {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 2px;
    flex-shrink: 0;
  }

  .legend-name {
    color: var(--text);
    white-space: nowrap;
    text-transform: capitalize;
  }

  .legend-note {
    color: var(--text-muted);
    font-style: italic;
  }

  .legend-gradient-row {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  .legend-gradient {
    width: 60px;
    height: 10px;
    border-radius: 2px;
  }

  .legend-gradient.depth-gradient {
    background: linear-gradient(to right, #deebf7, #9ecae1, #4292c6, #2171b5, #084594);
  }

  .legend-gradient.fanout-gradient {
    background: linear-gradient(to right, #1a9850, #91cf60, #d9ef8b, #fee08b, #fc8d59, #d73027);
  }

  .legend-gradient-label {
    color: var(--text-muted);
    font-size: 0.65rem;
  }

  .sunburst-tooltip {
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
    margin-bottom: 0.3rem;
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

  .center-message {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: 1rem;
    height: 100%;
  }
</style>
