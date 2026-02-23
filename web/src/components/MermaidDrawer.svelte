<script lang="ts">
  import { onMount } from "svelte";
  import { mermaidStore, type DiagramType } from "../stores/mermaid.svelte";
  import type { CytoscapeGraph } from "../lib/types";
  import { generateFlowchart, generateDataFlow, generateSequence, generateC4 } from "../lib/mermaid-gen";

  interface Props {
    graph: CytoscapeGraph | null;
  }

  let { graph }: Props = $props();

  let renderContainer = $state<HTMLDivElement>();
  let mermaidReady = $state(false);
  let renderError = $state<string | null>(null);
  let mermaidModule: typeof import("mermaid") | null = null;

  // Lazily load and initialize mermaid
  onMount(async () => {
    try {
      mermaidModule = await import("mermaid");
      mermaidModule.default.initialize({
        startOnLoad: false,
        theme: "dark",
        securityLevel: "loose",
        flowchart: { useMaxWidth: true, htmlLabels: true },
        sequence: { useMaxWidth: true },
      });
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

  // Render the diagram when source changes
  $effect(() => {
    const src = source;
    const ready = mermaidReady;
    if (!ready || !src || !renderContainer || !mermaidModule) return;

    renderError = null;
    // Use a unique ID to avoid mermaid caching issues
    const id = `mermaid-${Date.now()}`;
    renderContainer.innerHTML = "";

    mermaidModule.default
      .render(id, src)
      .then(({ svg }) => {
        if (renderContainer) {
          renderContainer.innerHTML = svg;
        }
      })
      .catch((err: unknown) => {
        renderError = `Render error: ${err instanceof Error ? err.message : String(err)}`;
        // Clean up any mermaid error elements
        const errorEl = document.getElementById("d" + id);
        if (errorEl) errorEl.remove();
      });
  });

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

{#if mermaidStore.open}
  <div class="mermaid-drawer">
    <div class="drawer-header">
      <span class="drawer-title">Mermaid Diagram</span>
      <div class="drawer-controls">
        <select
          bind:value={mermaidStore.diagramType}
          aria-label="Diagram type"
        >
          {#each DIAGRAM_OPTIONS as opt}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
        <button class="copy-btn" onclick={copySource} title="Copy Mermaid source">Copy</button>
        <button class="close-btn" onclick={() => mermaidStore.close()} aria-label="Close drawer">&times;</button>
      </div>
    </div>

    <div class="drawer-content">
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
      <div class="render-container" bind:this={renderContainer}></div>
    </div>
  </div>
{/if}

<style>
  .mermaid-drawer {
    position: absolute;
    top: 0;
    right: 0;
    width: 560px;
    max-width: 100%;
    height: 100%;
    background: var(--surface);
    border-left: 1px solid var(--border);
    box-shadow: -4px 0 16px rgba(0, 0, 0, 0.2);
    z-index: 20;
    display: flex;
    flex-direction: column;
  }

  .drawer-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .drawer-title {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text);
  }

  .drawer-controls {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .drawer-controls select {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    padding: 0.2rem 0.4rem;
    border-radius: 3px;
    font-size: 0.8rem;
  }

  .copy-btn {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    font-size: 0.75rem;
    padding: 0.2rem 0.5rem;
    border-radius: 3px;
    cursor: pointer;
  }

  .copy-btn:hover {
    background: var(--border);
  }

  .close-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 1.2rem;
    cursor: pointer;
    padding: 0.1rem 0.3rem;
    line-height: 1;
    border-radius: 3px;
  }

  .close-btn:hover {
    color: var(--text);
    background: var(--bg);
  }

  .drawer-content {
    flex: 1;
    overflow: auto;
    padding: 0.5rem;
  }

  .render-container {
    display: flex;
    justify-content: center;
    min-height: 100px;
  }

  .render-container :global(svg) {
    max-width: 100%;
    height: auto;
  }

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
</style>
