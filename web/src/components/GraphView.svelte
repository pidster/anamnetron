<script lang="ts">
  import { onMount } from "svelte";
  import cytoscape from "cytoscape";
  import coseBilkent from "cytoscape-cose-bilkent";
  import dagre from "cytoscape-dagre";
  import type { CytoscapeGraph, ConformanceReport, SnapshotDiff } from "../lib/types";
  import { selectionStore } from "../stores/selection.svelte";
  import { buildTraversalIndex, type TraversalIndex } from "../lib/traversal";

  // Register layout extensions once
  cytoscape.use(coseBilkent);
  cytoscape.use(dagre);

  interface Props {
    graph: CytoscapeGraph | null;
    conformance?: ConformanceReport | null;
    diff?: SnapshotDiff | null;
    layout?: "cose-bilkent" | "dagre";
    theme?: "dark" | "light";
  }

  let { graph, conformance = null, diff = null, layout = "cose-bilkent", theme = "dark" }: Props = $props();

  let container: HTMLDivElement;
  let cy: cytoscape.Core | null = null;
  let traversalIndex: TraversalIndex | null = null;

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
      {
        selector: ".diff-added",
        style: { "border-color": pass, "border-width": 3, "border-style": "dashed" },
      },
      {
        selector: ".diff-removed",
        style: { "border-color": fail, "border-width": 3, "border-style": "dashed", opacity: 0.5 },
      },
      {
        selector: ".diff-changed",
        style: { "border-color": warn, "border-width": 3, "border-style": "dashed" },
      },
      {
        selector: "edge.diff-added",
        style: { "line-color": pass, "target-arrow-color": pass, "line-style": "dashed" },
      },
      {
        selector: "edge.diff-removed",
        style: { "line-color": fail, "target-arrow-color": fail, "line-style": "dashed", opacity: 0.5 },
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

  function applyDiffOverlay(report: SnapshotDiff) {
    if (!cy) return;

    // Clear previous diff overlay
    cy.elements().removeClass("diff-added diff-removed diff-changed");

    // Apply node changes
    for (const change of report.node_changes) {
      const node = cy.nodes().filter((n) => n.data("canonical_path") === change.canonical_path);
      if (node.length > 0) {
        node.addClass(`diff-${change.change}`);
      }
    }

    // Apply edge changes
    for (const change of report.edge_changes) {
      const edge = cy.edges().filter((e) => {
        const srcNode = cy!.getElementById(e.data("source"));
        const tgtNode = cy!.getElementById(e.data("target"));
        return (
          srcNode.data("canonical_path") === change.source_path &&
          tgtNode.data("canonical_path") === change.target_path &&
          e.data("kind") === change.edge_kind
        );
      });
      if (edge.length > 0) {
        edge.addClass(`diff-${change.change}`);
      }
    }
  }

  function clearDiffOverlay() {
    if (!cy) return;
    cy.elements().removeClass("diff-added diff-removed diff-changed");
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

  // Rebuild traversal index when graph changes
  $effect(() => {
    if (graph) {
      traversalIndex = buildTraversalIndex(graph);
    } else {
      traversalIndex = null;
    }
  });

  $effect(() => {
    if (conformance && cy) {
      applyConformanceOverlay(conformance);
    }
  });

  $effect(() => {
    if (diff && cy) {
      applyDiffOverlay(diff);
    } else if (!diff && cy) {
      clearDiffOverlay();
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

  /** Get the current traversal index for keyboard navigation. */
  export function getTraversalIndex(): TraversalIndex | null {
    return traversalIndex;
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
