<script lang="ts">
  import { treemap, treemapSquarify } from "d3-hierarchy";
  import type { HierarchyNode, HierarchyRectangularNode } from "d3-hierarchy";
  import { scaleSequential } from "d3-scale";
  import { interpolateRdYlGn, interpolateBlues } from "d3-scale-chromatic";
  import type { CytoscapeGraph } from "../lib/types";
  import { buildHierarchy, sumByMetric, getMetric, isTestNode, type TreeNode } from "../lib/hierarchy";
  import { selectionStore } from "../stores/selection.svelte";

  /** A switchable metric preset for the treemap. */
  interface MetricPreset {
    name: string;
    description: string;
    areaMetric: string;
    colourMetric: string;
    colourScale: "diverging" | "sequential" | "depth";
  }

  const PRESETS: MetricPreset[] = [
    {
      name: "Size landscape",
      description: "Area by lines of code, colour by depth",
      areaMetric: "loc",
      colourMetric: "depth",
      colourScale: "depth",
    },
    {
      name: "Coupling hotspots",
      description: "Area by lines of code, colour by outgoing dependencies",
      areaMetric: "loc",
      colourMetric: "fan_out",
      colourScale: "diverging",
    },
    {
      name: "Dependency hubs",
      description: "Area by incoming dependencies, colour by outgoing",
      areaMetric: "fan_in",
      colourMetric: "fan_out",
      colourScale: "diverging",
    },
    {
      name: "Complexity hotspots",
      description: "Area by lines of code, colour by cyclomatic complexity",
      areaMetric: "loc",
      colourMetric: "cyclomatic",
      colourScale: "diverging",
    },
    {
      name: "Cognitive load",
      description: "Area by lines of code, colour by cognitive complexity",
      areaMetric: "loc",
      colourMetric: "cognitive",
      colourScale: "diverging",
    },
    {
      name: "Maintainability",
      description: "Area by lines of code, colour by maintainability index",
      areaMetric: "loc",
      colourMetric: "mi",
      colourScale: "diverging",
    },
  ];

  interface Props {
    graph: CytoscapeGraph | null;
    onselectnode?: (nodeId: string) => void;
  }

  let { graph, onselectnode }: Props = $props();

  let containerWidth = $state(800);
  let containerHeight = $state(600);

  // Active preset index
  let activePresetIndex = $state(1); // Default to "Coupling hotspots" (matches prior behaviour)

  let activePreset = $derived(PRESETS[activePresetIndex]);

  // Tooltip state
  let tooltip = $state<{
    visible: boolean;
    x: number;
    y: number;
    node: TreeNode | null;
    value: number;
  }>({ visible: false, x: 0, y: 0, node: null, value: 0 });

  // Build the full (unsummed) hierarchy from graph data
  let fullHierarchy = $derived.by((): HierarchyNode<TreeNode> | null => {
    if (!graph) return null;
    return buildHierarchy(graph);
  });

  // Check which metrics are available in the data
  let availableMetrics = $derived.by((): Set<string> => {
    const metrics = new Set<string>();
    // "depth" and "count" are always available
    metrics.add("depth");
    metrics.add("count");
    if (!fullHierarchy) return metrics;
    fullHierarchy.each((node) => {
      if (node.data.metadata) {
        for (const key of Object.keys(node.data.metadata)) {
          if (typeof node.data.metadata[key] === "number") {
            metrics.add(key);
          }
        }
      }
    });
    return metrics;
  });

  /** Check if a preset has all required metrics available. */
  function isPresetAvailable(preset: MetricPreset): boolean {
    return (
      availableMetrics.has(preset.areaMetric) &&
      availableMetrics.has(preset.colourMetric)
    );
  }

  // The treemap root is always the full hierarchy root (focus is handled by App.svelte)
  let drillRoot = $derived(fullHierarchy);

  // Compute maximum value for the colour metric across all leaves
  let maxColourValue = $derived.by((): number => {
    if (!fullHierarchy) return 1;
    const metric = activePreset.colourMetric;
    if (metric === "depth") {
      let maxDepth = 1;
      fullHierarchy.each((node) => {
        if (node.depth > maxDepth) maxDepth = node.depth;
      });
      return maxDepth;
    }
    let max = 1;
    fullHierarchy.each((node) => {
      const val = getMetric(node.data, metric);
      if (val > max) max = val;
    });
    return max;
  });

  // Colour scale based on active preset
  let colorScale = $derived.by(() => {
    if (activePreset.colourScale === "depth") {
      return scaleSequential(interpolateBlues).domain([0, maxColourValue]);
    }
    // "diverging" and "sequential" both use RdYlGn reversed
    return scaleSequential((t: number) => interpolateRdYlGn(1 - t)).domain([0, maxColourValue]);
  });

  // Compute treemap layout rectangles
  let rectangles = $derived.by((): HierarchyRectangularNode<TreeNode>[] => {
    if (!drillRoot || containerWidth <= 0 || containerHeight <= 0) return [];

    // Re-sum the drill root using the active preset's area metric
    const root = sumByMetric(drillRoot.copy(), activePreset.areaMetric);

    const layout = treemap<TreeNode>()
      .size([containerWidth, containerHeight])
      .tile(treemapSquarify)
      .padding(2)
      .round(true);

    const laid = layout(root);
    return laid.leaves();
  });

  // Legend labels derived from preset
  let legendColourLabel = $derived(
    activePreset.colourMetric === "depth"
      ? "Depth"
      : activePreset.colourMetric === "fan_out"
        ? "Fan-out"
        : activePreset.colourMetric === "fan_in"
          ? "Fan-in"
          : activePreset.colourMetric,
  );

  let legendAreaLabel = $derived(
    activePreset.areaMetric === "loc"
      ? "LOC"
      : activePreset.areaMetric === "fan_in"
        ? "fan-in"
        : activePreset.areaMetric === "fan_out"
          ? "fan-out"
          : activePreset.areaMetric === "count"
            ? "count"
            : activePreset.areaMetric,
  );

  // The currently selected node ID from the shared selection store
  let externalSelectedId = $derived(selectionStore.selectedNodeId);

  function getColor(node: HierarchyRectangularNode<TreeNode>): string {
    const metric = activePreset.colourMetric;
    if (metric === "depth") {
      return colorScale(node.depth);
    }
    const val = getMetric(node.data, metric);
    return colorScale(val);
  }

  function handleClick(node: HierarchyRectangularNode<TreeNode>) {
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

  function shouldShowLabel(node: HierarchyRectangularNode<TreeNode>): boolean {
    const width = node.x1 - node.x0;
    const height = node.y1 - node.y0;
    return width > 40 && height > 16;
  }

  function truncateLabel(
    label: string,
    node: HierarchyRectangularNode<TreeNode>,
  ): string {
    const width = node.x1 - node.x0;
    // Rough estimate: ~7px per character
    const maxChars = Math.floor((width - 8) / 7);
    if (maxChars <= 0) return "";
    if (label.length <= maxChars) return label;
    return label.slice(0, maxChars - 1) + "\u2026";
  }
</script>

<div
  class="treemap-container"
  bind:clientWidth={containerWidth}
  bind:clientHeight={containerHeight}
>
  <div class="preset-toolbar">
    {#each PRESETS as preset, i}
      {@const available = isPresetAvailable(preset)}
      <button
        class="preset-btn"
        class:active={i === activePresetIndex}
        disabled={!available}
        title={available ? preset.description : `Unavailable: requires ${preset.areaMetric} and ${preset.colourMetric} metrics`}
        onclick={() => { if (available) activePresetIndex = i; }}
      >
        {preset.name}
      </button>
    {/each}
  </div>

  <div class="treemap-area" role="img" aria-label="Treemap visualisation">
    {#each rectangles as rect (rect.data.id)}
      {@const width = rect.x1 - rect.x0}
      {@const height = rect.y1 - rect.y0}
      <div
        class="treemap-cell"
        class:treemap-cell-selected={externalSelectedId === rect.data.id}
        style="
          left: {rect.x0}px;
          top: {rect.y0}px;
          width: {width}px;
          height: {height}px;
          background-color: {getColor(rect)};
          {isTestNode(rect.data) ? 'opacity: 0.5;' : ''}
        "
        role="button"
        tabindex="0"
        onclick={() => handleClick(rect)}
        onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') handleClick(rect); }}
        onmouseenter={(e) => handleMouseEnter(e, rect)}
        onmousemove={handleMouseMove}
        onmouseleave={handleMouseLeave}
      >
        {#if shouldShowLabel(rect)}
          <span class="treemap-label">
            {truncateLabel(rect.data.label, rect)}
          </span>
        {/if}
      </div>
    {/each}

    {#if rectangles.length === 0 && graph}
      <div class="center-message">
        <p>No nodes to display in treemap</p>
      </div>
    {/if}
  </div>

  {#if tooltip.visible && tooltip.node}
    <div
      class="treemap-tooltip"
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
      {#if getMetric(tooltip.node, "cyclomatic") > 0}
        <div class="tooltip-row">
          <span class="tooltip-key">Cyclomatic</span>
          <span>{getMetric(tooltip.node, "cyclomatic")}</span>
        </div>
      {/if}
      {#if getMetric(tooltip.node, "cognitive") > 0}
        <div class="tooltip-row">
          <span class="tooltip-key">Cognitive</span>
          <span>{getMetric(tooltip.node, "cognitive")}</span>
        </div>
      {/if}
      {#if getMetric(tooltip.node, "mi") > 0}
        <div class="tooltip-row">
          <span class="tooltip-key">MI</span>
          <span>{getMetric(tooltip.node, "mi").toFixed(1)}</span>
        </div>
      {/if}
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

  <div class="treemap-legend">
    <span class="legend-label">{legendColourLabel}:</span>
    <span class="legend-low">Low</span>
    <div class="legend-gradient" class:depth-gradient={activePreset.colourScale === "depth"}></div>
    <span class="legend-high">High</span>
    <span class="legend-note">(area = {legendAreaLabel})</span>
  </div>
</div>

<style>
  .treemap-container {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
    position: relative;
    overflow: hidden;
  }

  .preset-toolbar {
    display: flex;
    align-items: center;
    padding: 0.35rem 0.75rem;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    gap: 0.35rem;
    flex-shrink: 0;
  }

  .preset-btn {
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text-muted);
    cursor: pointer;
    font-size: 0.75rem;
    padding: 0.2rem 0.6rem;
    border-radius: 4px;
    transition: background 0.15s ease, color 0.15s ease, border-color 0.15s ease;
  }

  .preset-btn:hover:not(:disabled) {
    background: var(--border);
    color: var(--text);
  }

  .preset-btn.active {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
    cursor: default;
  }

  .preset-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .treemap-area {
    flex: 1;
    position: relative;
    min-height: 0;
  }

  .treemap-cell {
    position: absolute;
    overflow: hidden;
    cursor: pointer;
    border: 1px solid rgba(0, 0, 0, 0.25);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: opacity 0.15s ease;
  }

  .treemap-cell:hover {
    opacity: 0.85;
    border-color: var(--text);
    z-index: 1;
  }

  .treemap-cell:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
    z-index: 2;
  }

  .treemap-cell-selected {
    border: 2px solid var(--accent, #5b9bd5);
    box-shadow: 0 0 0 2px var(--accent, #5b9bd5);
    z-index: 3;
  }

  .treemap-label {
    color: #fff;
    font-size: 0.7rem;
    font-weight: 500;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.6);
    pointer-events: none;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    padding: 0 4px;
    max-width: 100%;
  }

  .treemap-tooltip {
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

  .treemap-legend {
    position: absolute;
    bottom: 8px;
    right: 8px;
    display: flex;
    align-items: center;
    gap: 0.35rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.25rem 0.5rem;
    font-size: 0.7rem;
    color: var(--text-muted);
    opacity: 0.9;
  }

  .legend-label {
    font-weight: 600;
    color: var(--text);
  }

  .legend-gradient {
    width: 60px;
    height: 10px;
    border-radius: 2px;
    background: linear-gradient(
      to right,
      #1a9850,
      #91cf60,
      #d9ef8b,
      #fee08b,
      #fc8d59,
      #d73027
    );
  }

  .legend-gradient.depth-gradient {
    background: linear-gradient(
      to right,
      #deebf7,
      #9ecae1,
      #4292c6,
      #2171b5,
      #084594
    );
  }

  .legend-low {
    color: #1a9850;
    font-weight: 500;
  }

  .legend-high {
    color: #d73027;
    font-weight: 500;
  }

  .legend-note {
    font-style: italic;
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
