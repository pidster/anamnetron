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
  }

  let { graph, conformance = null, layout = "cose-bilkent" }: Props = $props();

  let container: HTMLDivElement;
  let cy: cytoscape.Core | null = null;

  const styleSheet: cytoscape.Stylesheet[] = [
    {
      selector: "node",
      style: {
        label: "data(label)",
        "text-valign": "center",
        "text-halign": "center",
        "background-color": "#53a8b6",
        color: "#fff",
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
        "background-color": "#16213e",
        "background-opacity": 0.6,
        "border-color": "#0f3460",
        "border-width": 2,
        "text-valign": "top",
        "text-halign": "center",
        "font-size": "14px",
        "font-weight": "bold",
        padding: "20px",
      },
    },
    {
      selector: "edge",
      style: {
        width: 2,
        "line-color": "#607d8b",
        "target-arrow-color": "#607d8b",
        "target-arrow-shape": "triangle",
        "curve-style": "bezier",
        "arrow-scale": 0.8,
      },
    },
    {
      selector: "edge[kind = 'depends']",
      style: { "line-style": "solid", "line-color": "#53a8b6", "target-arrow-color": "#53a8b6" },
    },
    {
      selector: "edge[kind = 'data_flow']",
      style: { "line-style": "dashed", "line-color": "#ff9800", "target-arrow-color": "#ff9800" },
    },
    {
      selector: "edge[kind = 'implements']",
      style: { "line-style": "dotted", "line-color": "#4caf50", "target-arrow-color": "#4caf50" },
    },
    {
      selector: "node:selected",
      style: { "border-color": "#fff", "border-width": 3 },
    },
    // Conformance overlay classes
    {
      selector: ".conformance-pass",
      style: { "border-color": "#4caf50", "border-width": 3 },
    },
    {
      selector: ".conformance-fail",
      style: { "border-color": "#f44336", "border-width": 3 },
    },
    {
      selector: ".conformance-unimplemented",
      style: { "border-color": "#ff9800", "border-width": 3 },
    },
    {
      selector: ".conformance-undocumented",
      style: { "border-color": "#607d8b", "border-width": 3 },
    },
  ];

  function initCytoscape(elements: CytoscapeGraph["elements"]) {
    if (cy) cy.destroy();

    cy = cytoscape({
      container,
      elements: {
        nodes: elements.nodes,
        edges: elements.edges,
      },
      style: styleSheet,
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
