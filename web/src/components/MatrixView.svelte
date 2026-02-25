<script lang="ts">
  import { scaleSequential } from "d3-scale";
  import { interpolateBlues } from "d3-scale-chromatic";
  import type { CytoscapeGraph } from "../lib/types";
  import {
    buildHierarchicalMatrix,
    type MatrixSortMode,
    type MatrixNode,
    type MatrixCell,
    type HierarchicalMatrix,
  } from "../lib/hierarchical-matrix";
  import { selectionStore } from "../stores/selection.svelte";

  interface Props {
    graph: CytoscapeGraph | null;
  }

  let { graph }: Props = $props();

  let sortMode = $state<MatrixSortMode>("hierarchy");

  // Own expansion state (separate from global expansionStore)
  let matrixExpanded = $state<Set<string>>(new Set());

  // Hover state
  let hoveredRow = $state<number | null>(null);
  let hoveredCol = $state<number | null>(null);

  // Initialize default expansion when graph changes
  $effect(() => {
    if (!graph) {
      matrixExpanded = new Set();
      return;
    }
    // Start collapsed at service level: expand system nodes only
    const systemNodes = new Set<string>();
    for (const node of graph.elements.nodes) {
      if (node.data.kind === "system") {
        systemNodes.add(node.data.id);
      }
    }
    matrixExpanded = systemNodes;
  });

  let matrixData = $derived.by((): HierarchicalMatrix => {
    if (!graph) return { nodes: [], cells: [], maxCount: 0 };
    return buildHierarchicalMatrix(graph, matrixExpanded, sortMode);
  });

  let colorScale = $derived.by(() => {
    return scaleSequential(interpolateBlues).domain([0, Math.max(matrixData.maxCount, 1)]);
  });

  // Build a fast cell lookup: "row,col" -> MatrixCell
  let cellLookup = $derived.by(() => {
    const map = new Map<string, MatrixCell>();
    for (const cell of matrixData.cells) {
      map.set(`${cell.row},${cell.col}`, cell);
    }
    return map;
  });

  function toggleExpand(nodeId: string) {
    const next = new Set(matrixExpanded);
    if (next.has(nodeId)) {
      next.delete(nodeId);
    } else {
      next.add(nodeId);
    }
    matrixExpanded = next;
  }

  function handleCellClick(row: number, _col: number) {
    const node = matrixData.nodes[row];
    if (node) {
      selectionStore.selectSingle(node.id);
      selectionStore.panelOpen = true;
    }
  }

  function handleHeaderClick(nodeId: string) {
    selectionStore.selectSingle(nodeId);
    selectionStore.panelOpen = true;
  }

  function getCellBackground(row: number, col: number): string {
    const cell = cellLookup.get(`${row},${col}`);
    if (!cell) return "transparent";
    return colorScale(cell.count);
  }

  function getCellText(row: number, col: number): string {
    const cell = cellLookup.get(`${row},${col}`);
    if (!cell) return "";
    return String(cell.count);
  }

  function getCellBorder(row: number, col: number): string {
    const cell = cellLookup.get(`${row},${col}`);
    if (cell?.isCyclic) return "2px solid var(--fail, #f44336)";
    return "1px solid var(--border)";
  }

  function isHighlighted(row: number, col: number): boolean {
    return row === hoveredRow || col === hoveredCol;
  }

  function isCellDark(row: number, col: number): boolean {
    const cell = cellLookup.get(`${row},${col}`);
    if (!cell) return false;
    return cell.count > matrixData.maxCount * 0.5;
  }
</script>

<div class="matrix-container">
  <div class="matrix-toolbar">
    <span class="toolbar-label">Sort:</span>
    {#each [
      { value: "hierarchy" as MatrixSortMode, label: "Hierarchy" },
      { value: "alphabetical" as MatrixSortMode, label: "A-Z" },
      { value: "dependency-count" as MatrixSortMode, label: "Dependencies" },
    ] as item}
      <button
        class="sort-btn"
        class:sort-btn-active={sortMode === item.value}
        onclick={() => { sortMode = item.value; }}
      >{item.label}</button>
    {/each}
  </div>

  {#if matrixData.nodes.length === 0}
    <div class="center-message">
      <p>No nodes to display in matrix</p>
    </div>
  {:else}
    <div class="matrix-scroll">
      <div
        class="matrix-grid"
        style="--cols: {matrixData.nodes.length + 1};"
        role="grid"
        aria-label="Dependency Structure Matrix"
      >
        <!-- Corner cell -->
        <div class="matrix-corner"></div>

        <!-- Column headers (rotated) -->
        {#each matrixData.nodes as node, col}
          <div
            class="matrix-col-header"
            class:matrix-header-highlighted={col === hoveredCol}
            role="columnheader"
          >
            <button
              class="header-btn col-header-btn"
              onclick={() => handleHeaderClick(node.id)}
              title={node.label}
            >
              {node.label}
            </button>
          </div>
        {/each}

        <!-- Rows -->
        {#each matrixData.nodes as rowNode, row}
          <!-- Row header -->
          <div
            class="matrix-row-header"
            class:matrix-header-highlighted={row === hoveredRow}
            style="padding-left: {rowNode.depth * 16 + 4}px;"
            role="rowheader"
          >
            {#if rowNode.hasChildren}
              <button
                class="expand-toggle"
                onclick={() => toggleExpand(rowNode.id)}
                aria-label={rowNode.expanded ? "Collapse" : "Expand"}
              >
                {rowNode.expanded ? "\u25BC" : "\u25B6"}
              </button>
            {:else}
              <span class="expand-spacer"></span>
            {/if}
            <button
              class="header-btn row-header-btn"
              onclick={() => handleHeaderClick(rowNode.id)}
              title={rowNode.label}
            >
              {rowNode.label}
            </button>
          </div>

          <!-- Row cells -->
          {#each matrixData.nodes as _, col}
            <div
              class="matrix-cell"
              class:matrix-cell-highlighted={isHighlighted(row, col)}
              class:matrix-cell-diagonal={row === col}
              style="background: {row === col ? 'var(--surface)' : getCellBackground(row, col)}; border: {getCellBorder(row, col)};"
              role="gridcell"
              tabindex="0"
              aria-label="Row {rowNode.label}, Column {matrixData.nodes[col].label}: {getCellText(row, col) || '0'}"
              onclick={() => handleCellClick(row, col)}
              onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") handleCellClick(row, col); }}
              onmouseenter={() => { hoveredRow = row; hoveredCol = col; }}
              onmouseleave={() => { hoveredRow = null; hoveredCol = null; }}
            >
              {#if row !== col}
                <span
                  class="cell-text"
                  class:cell-text-light={isCellDark(row, col)}
                >
                  {getCellText(row, col)}
                </span>
              {/if}
            </div>
          {/each}
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .matrix-container {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }

  .matrix-toolbar {
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

  .sort-btn {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    font-size: 0.75rem;
    padding: 0.2rem 0.5rem;
    border-radius: 3px;
    cursor: pointer;
  }

  .sort-btn-active {
    background: var(--accent);
    color: #fff;
    border-color: var(--accent);
  }

  .matrix-scroll {
    flex: 1;
    overflow: auto;
    min-height: 0;
  }

  .matrix-grid {
    display: grid;
    grid-template-columns: minmax(140px, auto) repeat(var(--cols, 1), minmax(28px, 1fr));
    width: fit-content;
    min-width: 100%;
  }

  .matrix-corner {
    position: sticky;
    left: 0;
    top: 0;
    z-index: 3;
    background: var(--surface);
    border-bottom: 2px solid var(--border);
    border-right: 2px solid var(--border);
  }

  .matrix-col-header {
    position: sticky;
    top: 0;
    z-index: 2;
    background: var(--surface);
    border-bottom: 2px solid var(--border);
    min-height: 80px;
    display: flex;
    align-items: flex-end;
    justify-content: flex-start;
    padding: 4px 2px;
    overflow: hidden;
  }

  .matrix-col-header .col-header-btn {
    transform: rotate(-45deg);
    transform-origin: left bottom;
    white-space: nowrap;
    font-size: 0.7rem;
    max-width: 100px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .matrix-row-header {
    position: sticky;
    left: 0;
    z-index: 2;
    background: var(--surface);
    border-right: 2px solid var(--border);
    display: flex;
    align-items: center;
    gap: 2px;
    min-height: 28px;
    overflow: hidden;
    white-space: nowrap;
  }

  .matrix-header-highlighted {
    background: color-mix(in srgb, var(--accent) 15%, var(--surface)) !important;
  }

  .expand-toggle {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 0.6rem;
    padding: 0 2px;
    flex-shrink: 0;
    width: 14px;
    text-align: center;
  }

  .expand-toggle:hover {
    color: var(--text);
  }

  .expand-spacer {
    display: inline-block;
    width: 14px;
    flex-shrink: 0;
  }

  .header-btn {
    background: none;
    border: none;
    color: var(--text);
    cursor: pointer;
    font-size: 0.75rem;
    padding: 0.1rem 0.2rem;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .header-btn:hover {
    color: var(--accent);
  }

  .row-header-btn {
    max-width: 120px;
  }

  .matrix-cell {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 28px;
    min-width: 28px;
    cursor: pointer;
    transition: background 0.1s ease;
    font-size: 0.7rem;
  }

  .matrix-cell:hover {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
    z-index: 1;
  }

  .matrix-cell-highlighted {
    background: color-mix(in srgb, var(--accent) 8%, transparent) !important;
  }

  .matrix-cell-diagonal {
    cursor: default;
  }

  .cell-text {
    color: var(--text);
    font-variant-numeric: tabular-nums;
    font-weight: 500;
  }

  .cell-text-light {
    color: #fff;
  }

  .center-message {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: 1rem;
  }

  .center-message p {
    margin: 0.25rem 0;
  }
</style>
