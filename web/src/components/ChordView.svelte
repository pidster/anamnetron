<script lang="ts">
  import { chord as d3chord, ribbon as d3ribbon } from "d3-chord";
  import type { Chord, ChordGroup } from "d3-chord";
  import { arc as d3arc } from "d3-shape";
  import type { CytoscapeGraph } from "../lib/types";
  import { buildDependencyMatrix, type DependencyMatrix } from "../lib/dependency-matrix";
  import { selectionStore } from "../stores/selection.svelte";

  interface Props {
    graph: CytoscapeGraph | null;
    onselectnode?: (nodeId: string) => void;
  }

  let { graph, onselectnode }: Props = $props();

  let containerWidth = $state(800);
  let containerHeight = $state(600);
  let threshold = $state(0);
  let selectedArcIndex = $state<number | null>(null);
  let hoveredArcIndex = $state<number | null>(null);
  let hoveredChord = $state<{ source: number; target: number } | null>(null);
  let tooltipText = $state("");
  let tooltipX = $state(0);
  let tooltipY = $state(0);
  let tooltipVisible = $state(false);

  let matrix = $derived.by((): DependencyMatrix => {
    if (!graph) return { names: [], ids: [], colors: [], matrix: [] };
    return buildDependencyMatrix(graph);
  });

  // Apply threshold: zero out matrix cells below the threshold
  let thresholdedMatrix = $derived.by((): number[][] => {
    return matrix.matrix.map((row) =>
      row.map((val) => (val >= threshold ? val : 0)),
    );
  });

  // Compute max edge count for the threshold slider range
  let maxEdgeCount = $derived.by((): number => {
    let max = 0;
    for (const row of matrix.matrix) {
      for (const val of row) {
        if (val > max) max = val;
      }
    }
    return max;
  });

  let radius = $derived(Math.min(containerWidth, containerHeight) / 2 - 60);
  let innerRadius = $derived(Math.max(radius - 30, 10));

  let chordLayout = $derived.by(() => {
    if (thresholdedMatrix.length === 0) return null;
    const layout = d3chord()
      .padAngle(0.05)
      .sortSubgroups((a, b) => b - a);
    return layout(thresholdedMatrix);
  });

  // d3 generators typed broadly: innerRadius/outerRadius/radius are set on the generator,
  // so callers only need to provide angle properties.
  let arcPath = $derived.by(() => {
    const gen = d3arc().innerRadius(innerRadius).outerRadius(radius);
    return (group: ChordGroup): string => (gen as unknown as (d: ChordGroup) => string)(group);
  });

  let ribbonPath = $derived.by(() => {
    const gen = d3ribbon().radius(innerRadius);
    return (c: Chord): string => (gen as unknown as (d: Chord) => string)(c);
  });

  function isChordFaded(sourceIdx: number, targetIdx: number): boolean {
    if (hoveredArcIndex !== null) {
      return sourceIdx !== hoveredArcIndex && targetIdx !== hoveredArcIndex;
    }
    if (hoveredChord !== null) {
      return !(
        (sourceIdx === hoveredChord.source && targetIdx === hoveredChord.target) ||
        (sourceIdx === hoveredChord.target && targetIdx === hoveredChord.source)
      );
    }
    if (selectedArcIndex !== null) {
      return sourceIdx !== selectedArcIndex && targetIdx !== selectedArcIndex;
    }
    return false;
  }

  function isArcFaded(index: number): boolean {
    if (hoveredArcIndex !== null) {
      return index !== hoveredArcIndex;
    }
    if (selectedArcIndex !== null) {
      return index !== selectedArcIndex;
    }
    return false;
  }

  function handleArcEnter(index: number, event: MouseEvent) {
    hoveredArcIndex = index;
    tooltipText = matrix.names[index];
    updateTooltipPosition(event);
    tooltipVisible = true;
  }

  function handleArcLeave() {
    hoveredArcIndex = null;
    tooltipVisible = false;
  }

  function handleChordEnter(sourceIdx: number, targetIdx: number, event: MouseEvent) {
    hoveredChord = { source: sourceIdx, target: targetIdx };
    const srcName = matrix.names[sourceIdx];
    const tgtName = matrix.names[targetIdx];
    const fwd = matrix.matrix[sourceIdx][targetIdx];
    const rev = matrix.matrix[targetIdx][sourceIdx];
    const parts = [`${srcName} -> ${tgtName}: ${fwd}`];
    if (rev > 0) {
      parts.push(`${tgtName} -> ${srcName}: ${rev}`);
    }
    tooltipText = parts.join("\n");
    updateTooltipPosition(event);
    tooltipVisible = true;
  }

  function handleChordLeave() {
    hoveredChord = null;
    tooltipVisible = false;
  }

  function handleChordMove(event: MouseEvent) {
    updateTooltipPosition(event);
  }

  function handleArcMove(event: MouseEvent) {
    updateTooltipPosition(event);
  }

  function updateTooltipPosition(event: MouseEvent) {
    tooltipX = event.clientX;
    tooltipY = event.clientY;
  }

  function handleArcClick(index: number) {
    const nodeId = matrix.ids[index];
    if (nodeId) {
      onselectnode?.(nodeId);
    }
  }

  // Map from any node ID to its top-level module index (for external selection highlighting)
  let nodeToModuleIndex = $derived.by((): Map<string, number> => {
    if (!graph) return new Map();
    // Build parent lookup
    const parentMap = new Map<string, string | undefined>();
    for (const node of graph.elements.nodes) {
      parentMap.set(node.data.id, node.data.parent);
    }
    // Build ID-to-index map for the chord module IDs
    const idxMap = new Map<string, number>();
    for (let i = 0; i < matrix.ids.length; i++) {
      idxMap.set(matrix.ids[i], i);
    }
    // Map every node to its top-level module index
    const result = new Map<string, number>();
    for (const node of graph.elements.nodes) {
      const tlm = findTopLevelAncestor(node.data.id, parentMap);
      if (tlm !== null) {
        const idx = idxMap.get(tlm);
        if (idx !== undefined) {
          result.set(node.data.id, idx);
        }
      }
    }
    return result;
  });

  /** Walk up the containment hierarchy to find the top-level module (depth-1 node). */
  function findTopLevelAncestor(
    nodeId: string,
    parentMap: Map<string, string | undefined>,
  ): string | null {
    let current = nodeId;
    let parent = parentMap.get(current);
    while (parent !== undefined) {
      const grandparent = parentMap.get(parent);
      if (grandparent === undefined) return current;
      current = parent;
      parent = grandparent;
    }
    return null;
  }

  // React to external selection changes — highlight the arc containing the selected node
  $effect(() => {
    const nodeId = selectionStore.selectedNodeId;
    if (nodeId) {
      const idx = nodeToModuleIndex.get(nodeId);
      selectedArcIndex = idx ?? null;
    } else {
      selectedArcIndex = null;
    }
  });

  /** Compute label position for an arc group. */
  function labelTransform(group: { startAngle: number; endAngle: number }): string {
    const angle = (group.startAngle + group.endAngle) / 2;
    const labelRadius = radius + 16;
    const x = Math.sin(angle) * labelRadius;
    const y = -Math.cos(angle) * labelRadius;
    const degrees = (angle * 180) / Math.PI;
    // Flip text on the left side so it reads left-to-right
    const rotate = degrees > 180 ? degrees - 270 : degrees - 90;
    return `translate(${x},${y}) rotate(${rotate})`;
  }
</script>

<div class="chord-container" bind:clientWidth={containerWidth} bind:clientHeight={containerHeight}>
  {#if matrix.names.length === 0}
    <div class="center-message">
      <p>No module dependencies to display</p>
      <p class="hint">Load a snapshot with multiple top-level modules and dependency edges.</p>
    </div>
  {:else}
    <div class="chord-toolbar">
      <label class="threshold-label">
        Min edges:
        <input
          type="range"
          min="0"
          max={maxEdgeCount}
          bind:value={threshold}
          class="threshold-slider"
        />
        <span class="threshold-value">{threshold}</span>
      </label>
    </div>

    <svg
      viewBox="{-containerWidth / 2} {-containerHeight / 2} {containerWidth} {containerHeight}"
      class="chord-svg"
    >
      {#if chordLayout}
        <!-- Chords (ribbons) -->
        <g class="chords">
          {#each chordLayout as c}
            <path
              d={ribbonPath(c) ?? ""}
              fill={matrix.colors[c.source.index]}
              class="chord-ribbon"
              class:chord-faded={isChordFaded(c.source.index, c.target.index)}
              role="img"
              aria-label="{matrix.names[c.source.index]} to {matrix.names[c.target.index]}"
              onmouseenter={(e) => handleChordEnter(c.source.index, c.target.index, e)}
              onmouseleave={handleChordLeave}
              onmousemove={handleChordMove}
            />
          {/each}
        </g>

        <!-- Outer arcs -->
        <g class="arcs">
          {#each chordLayout.groups as group}
            <path
              d={arcPath(group) ?? ""}
              fill={matrix.colors[group.index]}
              class="chord-arc"
              class:chord-faded={isArcFaded(group.index)}
              class:chord-selected={selectedArcIndex === group.index}
              role="button"
              tabindex="0"
              aria-label="Module: {matrix.names[group.index]}"
              onmouseenter={(e) => handleArcEnter(group.index, e)}
              onmouseleave={handleArcLeave}
              onmousemove={handleArcMove}
              onclick={() => handleArcClick(group.index)}
              onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") handleArcClick(group.index); }}
            />
          {/each}
        </g>

        <!-- Labels -->
        <g class="labels">
          {#each chordLayout.groups as group}
            <text
              transform={labelTransform(group)}
              text-anchor="middle"
              class="chord-label"
              class:chord-faded={isArcFaded(group.index)}
              class:chord-label-selected={selectedArcIndex === group.index}
            >
              {matrix.names[group.index]}
            </text>
          {/each}
        </g>
      {/if}
    </svg>
  {/if}

  {#if matrix.names.length > 0}
    <div class="chord-legend">
      {#each matrix.names as name, i}
        <div class="legend-item">
          <span class="legend-swatch" style="background-color: {matrix.colors[i]}"></span>
          <span class="legend-name">{name}</span>
        </div>
      {/each}
    </div>
  {/if}

  {#if tooltipVisible}
    <div
      class="chord-tooltip"
      style="left: {tooltipX + 12}px; top: {tooltipY - 12}px;"
    >
      {#each tooltipText.split("\n") as line}
        <div>{line}</div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .chord-container {
    flex: 1;
    display: flex;
    flex-direction: column;
    position: relative;
    min-height: 0;
    overflow: hidden;
  }

  .chord-toolbar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 1rem;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .threshold-label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  .threshold-slider {
    width: 120px;
    accent-color: var(--accent);
  }

  .threshold-value {
    min-width: 1.5rem;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }

  .chord-svg {
    flex: 1;
    width: 100%;
    height: 100%;
  }

  .chord-ribbon {
    fill-opacity: 0.6;
    stroke: none;
    transition: fill-opacity 0.2s ease;
  }

  .chord-ribbon:hover {
    fill-opacity: 0.85;
  }

  .chord-ribbon.chord-faded {
    fill-opacity: 0.08;
  }

  .chord-arc {
    stroke: var(--bg);
    stroke-width: 1;
    cursor: pointer;
    transition: fill-opacity 0.2s ease;
    fill-opacity: 0.9;
  }

  .chord-arc:hover {
    fill-opacity: 1;
  }

  .chord-arc.chord-faded {
    fill-opacity: 0.3;
  }

  .chord-arc.chord-selected {
    stroke: var(--accent, #5b9bd5);
    stroke-width: 3;
    fill-opacity: 1;
  }

  .chord-label {
    font-size: 11px;
    fill: var(--text);
    pointer-events: none;
    transition: opacity 0.2s ease;
  }

  .chord-label.chord-faded {
    opacity: 0.3;
  }

  .chord-label-selected {
    font-weight: bold;
    fill: var(--accent, #5b9bd5);
  }

  .chord-legend {
    position: absolute;
    bottom: 8px;
    left: 8px;
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
  }

  .chord-tooltip {
    position: fixed;
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.4rem 0.6rem;
    font-size: 0.8rem;
    pointer-events: none;
    z-index: 1000;
    white-space: nowrap;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  }

  .center-message {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: 1.2rem;
  }

  .center-message p {
    margin: 0.25rem 0;
  }

  .hint {
    font-size: 0.9rem;
    color: var(--text-muted);
  }
</style>
