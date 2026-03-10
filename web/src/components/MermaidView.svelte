<script lang="ts">
  import { onMount } from "svelte";
  import DOMPurify from "dompurify";
  import { mermaidStore, type DiagramType } from "../stores/mermaid.svelte";
  import type { CytoscapeGraph } from "../lib/types";
  import { generateFlowchart, generateDataFlow, generateSequence, generateC4 } from "../lib/mermaid-gen";

  interface Props {
    graph: CytoscapeGraph | null;
    theme: "dark" | "light";
    totalNodeCount?: number;
    currentDepth?: number;
  }

  let { graph, theme, totalNodeCount = 0, currentDepth = 0 }: Props = $props();

  let visibleNodeCount = $derived(graph?.elements.nodes.length ?? 0);
  let hasHiddenNodes = $derived(totalNodeCount > 0 && visibleNodeCount < totalNodeCount);

  let renderContainer = $state<HTMLDivElement>();
  let mermaidReady = $state(false);
  let renderError = $state<string | null>(null);
  let zoom = $state(1);
  let mermaidModule: typeof import("mermaid") | null = null;

  const ZOOM_MIN = 0.25;
  const ZOOM_MAX = 4;
  const ZOOM_STEP = 0.1;

  // Lazily load mermaid
  onMount(async () => {
    try {
      mermaidModule = await import("mermaid");
      mermaidReady = true;
    } catch (e) {
      renderError = `Failed to load Mermaid: ${e instanceof Error ? e.message : String(e)}`;
    }
  });

  // Generate the source based on diagram type
  let source = $derived.by(() => {
    if (!graph) return "";
    switch (mermaidStore.diagramType) {
      case "flowchart": return generateFlowchart(graph);
      case "dataflow": return generateDataFlow(graph);
      case "sequence": return generateSequence(graph);
      case "c4": return generateC4(graph);
      default: return generateFlowchart(graph);
    }
  });

  // Keep store source in sync for copy button
  $effect(() => {
    mermaidStore.source = source;
  });

  // Re-initialize mermaid with correct theme and re-render when source or theme changes
  $effect(() => {
    const src = source;
    const ready = mermaidReady;
    const currentTheme = theme;
    if (!ready || !src || !renderContainer || !mermaidModule) return;

    // Re-initialize mermaid with current theme settings
    const isDark = currentTheme === "dark";
    mermaidModule.default.initialize({
      startOnLoad: false,
      theme: isDark ? "dark" : "default",
      securityLevel: "loose",
      maxTextSize: 200_000,
      // Let diagrams render at natural width so they aren't compressed
      flowchart: { useMaxWidth: false, htmlLabels: true },
      sequence: { useMaxWidth: false },
      // C4 diagram theming: Mermaid applies cScale colors to C4 component fills.
      // personBkg is used for the person-man class (which renders Component boxes).
      themeVariables: isDark ? {
        personBkg: "#2a2a2a",
        personBorder: "#5b9bd5",
      } : {
        personBkg: "#e3f2fd",
        personBorder: "#1976d2",
      },
    });

    renderError = null;
    const id = `mermaid-${Date.now()}`;
    renderContainer.innerHTML = "";

    mermaidModule.default
      .render(id, src)
      .then(({ svg }) => {
        if (renderContainer) {
          renderContainer.innerHTML = DOMPurify.sanitize(svg);
          // Fix C4 diagram contrast in dark mode.
          // Mermaid C4 uses inline fill/stroke attributes that CSS can't easily override.
          if (isDark && mermaidStore.diagramType === "c4") {
            patchC4DarkTheme(renderContainer);
          }
        }
      })
      .catch((err: unknown) => {
        renderError = `Render error: ${err instanceof Error ? err.message : String(err)}`;
        const errorEl = document.getElementById("d" + id);
        if (errorEl) errorEl.remove();
      });
  });

  /** Patch inline SVG attributes for C4 diagrams in dark mode. */
  function patchC4DarkTheme(container: HTMLDivElement) {
    // Component boxes: neutral dark fill, accent border
    container.querySelectorAll(".person-man rect").forEach((rect) => {
      rect.setAttribute("fill", "#2a2a2a");
      rect.setAttribute("stroke", "#5b9bd5");
    });
    // Component text: ensure readable
    container.querySelectorAll(".person-man text").forEach((text) => {
      text.setAttribute("fill", "#e8e8e8");
    });
    // Boundary rects: invisible #444444 → visible accent
    container.querySelectorAll('rect[stroke="#444444"]').forEach((rect) => {
      rect.setAttribute("stroke", "#5b9bd5");
    });
    // Boundary labels: dark grey → muted light
    container.querySelectorAll('text[fill="#444444"]').forEach((text) => {
      text.setAttribute("fill", "#999999");
    });
    // Relationship lines
    container.querySelectorAll('line[stroke="#444444"]').forEach((line) => {
      line.setAttribute("stroke", "#999999");
    });
    container.querySelectorAll('path[stroke="#444444"]').forEach((path) => {
      path.setAttribute("stroke", "#999999");
    });
  }

  // Reset zoom when diagram type changes
  $effect(() => {
    void mermaidStore.diagramType;
    zoom = 1;
  });

  function applyZoom(delta: number) {
    zoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, zoom + delta));
  }

  function handleWheel(e: WheelEvent) {
    if (!e.ctrlKey && !e.metaKey) return;
    e.preventDefault();
    applyZoom(e.deltaY < 0 ? ZOOM_STEP : -ZOOM_STEP);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "+" || e.key === "=") {
      e.preventDefault();
      applyZoom(ZOOM_STEP);
    } else if (e.key === "-") {
      e.preventDefault();
      applyZoom(-ZOOM_STEP);
    } else if (e.key === "0") {
      e.preventDefault();
      zoom = 1;
    }
  }

  function copySource() {
    void globalThis.navigator.clipboard?.writeText(source).catch(() => {});
  }

  const DIAGRAM_OPTIONS: Array<{ value: DiagramType; label: string }> = [
    { value: "flowchart", label: "Flowchart" },
    { value: "dataflow", label: "Data Flow" },
    { value: "sequence", label: "Sequence" },
    { value: "c4", label: "C4" },
  ];
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="mermaid-view" onkeydown={handleKeydown}>
  <div class="view-header">
    <span class="view-title">Mermaid Diagram</span>
    <div class="view-controls">
      <select
        bind:value={mermaidStore.diagramType}
        aria-label="Diagram type"
      >
        {#each DIAGRAM_OPTIONS as opt}
          <option value={opt.value}>{opt.label}</option>
        {/each}
      </select>
      <button class="icon-btn" onclick={() => applyZoom(-ZOOM_STEP)} title="Zoom out (-)">&#x2212;</button>
      <button
        class="zoom-label"
        onclick={() => { zoom = 1; }}
        title="Reset zoom (0)"
      >{Math.round(zoom * 100)}%</button>
      <button class="icon-btn" onclick={() => applyZoom(ZOOM_STEP)} title="Zoom in (+)">+</button>
      <button class="copy-btn" onclick={copySource} title="Copy Mermaid source">Copy</button>
    </div>
  </div>

  {#if hasHiddenNodes}
    <div class="info-bar">
      Showing {visibleNodeCount} of {totalNodeCount} nodes at depth {currentDepth}. Use depth controls or scope for more detail.
    </div>
  {/if}

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="view-content" onwheel={handleWheel}>
    {#if renderError}
      <div class="render-error">
        <p>{renderError}</p>
        <details>
          <summary>Source</summary>
          <pre>{source}</pre>
        </details>
      </div>
    {:else if !mermaidReady}
      <div class="loading">Loading Mermaid...</div>
    {/if}
    <div
      class="render-container"
      bind:this={renderContainer}
      style:transform="scale({zoom})"
      style:transform-origin="top left"
    ></div>
  </div>
</div>

<style>
  .mermaid-view {
    flex: 1;
    min-height: 0;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--surface);
  }

  .view-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .view-title {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text);
  }

  .view-controls {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .view-controls select {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    padding: 0.2rem 0.4rem;
    border-radius: 3px;
    font-size: 0.8rem;
  }

  .icon-btn,
  .copy-btn {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    font-size: 0.75rem;
    padding: 0.2rem 0.5rem;
    border-radius: 3px;
    cursor: pointer;
  }

  .zoom-label {
    background: var(--bg);
    color: var(--text-muted);
    border: 1px solid var(--border);
    font-size: 0.7rem;
    font-variant-numeric: tabular-nums;
    padding: 0.2rem 0.3rem;
    border-radius: 3px;
    cursor: pointer;
    min-width: 3rem;
    text-align: center;
  }

  .zoom-label:hover {
    color: var(--text);
    background: var(--border);
  }

  .icon-btn:hover,
  .copy-btn:hover {
    background: var(--border);
  }

  .view-content {
    flex: 1;
    overflow: auto;
    padding: 0.5rem;
  }

  .render-container {
    min-height: 100px;
    overflow: auto;
  }

  /* Let SVGs render at natural size and scroll instead of being compressed */
  .render-container :global(svg) {
    height: auto;
    min-width: min-content;
  }

  /* C4 dark mode patching is done via DOM manipulation in patchC4DarkTheme() */

  .render-error {
    padding: 0.5rem;
    color: var(--fail);
    font-size: 0.85rem;
  }

  .render-error pre {
    margin-top: 0.5rem;
    padding: 0.5rem;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 3px;
    font-size: 0.75rem;
    overflow-x: auto;
    color: var(--text-muted);
    white-space: pre-wrap;
    word-break: break-all;
  }

  .loading {
    padding: 1rem;
    text-align: center;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .info-bar {
    padding: 0.3rem 0.75rem;
    background: var(--bg);
    border-bottom: 1px solid var(--border);
    font-size: 0.8rem;
    color: var(--text-muted);
    flex-shrink: 0;
  }
</style>
