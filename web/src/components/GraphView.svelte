<script lang="ts">
  import { onMount } from "svelte";
  import cytoscape from "cytoscape";
  import coseBilkent from "cytoscape-cose-bilkent";
  import dagre from "cytoscape-dagre";
  import type { CytoscapeGraph, ConformanceReport } from "../lib/types";
  import { selectionStore } from "../stores/selection";

  // Register layout extensions once
  cytoscape.use(coseBilkent);
  cytoscape.use(dagre);

  interface Props {
    graph: CytoscapeGraph | null;
    conformance?: ConformanceReport | null;
    layout?: "cose-bilkent" | "dagre";
    theme?: "dark" | "light";
  }

  let { graph, conformance = null, layout = "cose-bilkent", theme = "dark" }: Props = $props();

  let container: HTMLDivElement;
  let cy: cytoscape.Core | null = null;

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

    return [
      {
        selector: "node",
        style: {
          label: "data(label)",
          "text-valign": "center",
          "text-halign": "center",
          "background-color": accent,
          color: isDark ? "#fff" : "#fff",
          "font-size": "12px",
          "text-wrap": "wrap",
          "text-max-width": "80px",
          width: "label",
          height: "label",
          padding: "10px",
          shape: "roundrectangle",
        },
      },
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
      {
        selector: "edge",
        style: {
          width: 2,
          "line-color": muted,
          "target-arrow-color": muted,
          "target-arrow-shape": "triangle",
          "curve-style": "bezier",
          "arrow-scale": 0.8,
        },
      },
      {
        selector: "edge[kind = 'depends']",
        style: { "line-style": "solid", "line-color": accent, "target-arrow-color": accent },
      },
      {
        selector: "edge[kind = 'data_flow']",
        style: { "line-style": "dashed", "line-color": warn, "target-arrow-color": warn },
      },
      {
        selector: "edge[kind = 'implements']",
        style: { "line-style": "dotted", "line-color": pass, "target-arrow-color": pass },
      },
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
    ];
  }

  function initCytoscape(elements: CytoscapeGraph["elements"]) {
    if (cy) cy.destroy();

    cy = cytoscape({
      container,
      elements: {
        nodes: elements.nodes,
        edges: elements.edges,
      },
      style: buildStyleSheet(),
      layout: {
        name: layout,
        animate: false,
        nodeDimensionsIncludeLabels: true,
      } as cytoscape.LayoutOptions,
    });

    cy.on("tap", "node", (evt) => {
      const nodeId = evt.target.id();
      selectionStore.selectedNodeId = nodeId;
      selectionStore.panelOpen = true;
    });

    cy.on("tap", (evt) => {
      if (evt.target === cy) {
        selectionStore.clear();
      }
    });
  }

  function applyConformanceOverlay(report: ConformanceReport) {
    if (!cy) return;

    // Clear previous overlay
    cy.nodes().removeClass(
      "conformance-pass conformance-fail conformance-unimplemented conformance-undocumented",
    );

    // Mark failed constraints
    for (const result of report.constraint_results) {
      if (result.status === "fail") {
        for (const violation of result.violations) {
          const node = cy.nodes().filter((n) => n.data("canonical_path") === violation.source_path);
          node.addClass("conformance-fail");
        }
      }
    }

    // Mark unimplemented
    for (const node of report.unimplemented) {
      cy.nodes()
        .filter((n) => n.data("canonical_path") === node.canonical_path)
        .addClass("conformance-unimplemented");
    }

    // Mark undocumented
    for (const node of report.undocumented) {
      cy.nodes()
        .filter((n) => n.data("canonical_path") === node.canonical_path)
        .addClass("conformance-undocumented");
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

  onMount(() => {
    return () => {
      if (cy) cy.destroy();
    };
  });

  $effect(() => {
    if (graph && container) {
      initCytoscape(graph.elements);
    }
  });

  $effect(() => {
    if (conformance && cy) {
      applyConformanceOverlay(conformance);
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

  /** Re-run layout. */
  export function relayout(name?: "cose-bilkent" | "dagre") {
    if (!cy) return;
    cy.layout({
      name: name || layout,
      animate: true,
      nodeDimensionsIncludeLabels: true,
    } as cytoscape.LayoutOptions).run();
  }
</script>

<div class="graph-container" bind:this={container}></div>

<style>
  .graph-container {
    flex: 1;
    min-height: 0;
    background: var(--bg);
  }
</style>
