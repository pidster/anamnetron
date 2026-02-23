import type { CytoscapeGraph } from "./types";

/**
 * Escape a string for use as a Mermaid node label.
 * Wraps in quotes and escapes inner quotes.
 */
function escapeLabel(label: string): string {
  return `"${label.replace(/"/g, "#quot;")}"`;
}

/**
 * Sanitize a node ID for Mermaid compatibility.
 * Mermaid IDs must be alphanumeric or underscores; replace other characters.
 */
function sanitizeId(id: string): string {
  return id.replace(/[^a-zA-Z0-9_]/g, "_");
}

/**
 * Generate a Mermaid flowchart from a CytoscapeGraph.
 *
 * Nodes are rendered as boxes, non-contains edges as arrows with kind labels.
 * Contains edges (parent-child) are expressed via subgraph nesting.
 */
export function generateFlowchart(graph: CytoscapeGraph): string {
  const lines: string[] = ["flowchart TD"];

  // Build parent-children map
  const children = new Map<string, string[]>();
  const rootNodes: string[] = [];

  for (const node of graph.elements.nodes) {
    if (node.data.parent) {
      const siblings = children.get(node.data.parent) ?? [];
      siblings.push(node.data.id);
      children.set(node.data.parent, siblings);
    } else {
      rootNodes.push(node.data.id);
    }
  }

  // Label lookup
  const labelOf = new Map<string, string>();
  for (const node of graph.elements.nodes) {
    labelOf.set(node.data.id, node.data.label);
  }

  // Render subgraphs recursively
  function renderNode(nodeId: string, indent: string): void {
    const kids = children.get(nodeId);
    const label = labelOf.get(nodeId) ?? nodeId;
    const sid = sanitizeId(nodeId);

    if (kids && kids.length > 0) {
      lines.push(`${indent}subgraph ${sid}[${escapeLabel(label)}]`);
      for (const child of kids) {
        renderNode(child, indent + "    ");
      }
      lines.push(`${indent}end`);
    } else {
      lines.push(`${indent}${sid}[${escapeLabel(label)}]`);
    }
  }

  for (const root of rootNodes) {
    renderNode(root, "    ");
  }

  // Render edges (skip contains edges and meta-edges)
  for (const edge of graph.elements.edges) {
    if (edge.data.kind === "contains") continue;
    if (edge.data._isMeta) continue;

    const src = sanitizeId(edge.data.source);
    const tgt = sanitizeId(edge.data.target);
    const kind = edge.data.kind;
    lines.push(`    ${src} -->|${kind}| ${tgt}`);
  }

  return lines.join("\n");
}

/**
 * Generate a Mermaid data-flow diagram.
 *
 * Shows only data_flow and calls edges, representing the runtime
 * interaction between components.
 */
export function generateDataFlow(graph: CytoscapeGraph): string {
  const dataFlowKinds = new Set(["data_flow", "calls"]);
  const lines: string[] = ["flowchart LR"];

  // Collect node IDs involved in data flow edges
  const involvedNodes = new Set<string>();
  const relevantEdges = graph.elements.edges.filter((e) => {
    if (e.data._isMeta) return false;
    return dataFlowKinds.has(e.data.kind);
  });

  for (const edge of relevantEdges) {
    involvedNodes.add(edge.data.source);
    involvedNodes.add(edge.data.target);
  }

  // Render only involved nodes
  const labelOf = new Map<string, string>();
  for (const node of graph.elements.nodes) {
    labelOf.set(node.data.id, node.data.label);
  }

  for (const nodeId of involvedNodes) {
    const label = labelOf.get(nodeId) ?? nodeId;
    lines.push(`    ${sanitizeId(nodeId)}[${escapeLabel(label)}]`);
  }

  // Render edges
  for (const edge of relevantEdges) {
    const src = sanitizeId(edge.data.source);
    const tgt = sanitizeId(edge.data.target);
    const style = edge.data.kind === "data_flow" ? "-.->|data_flow|" : "-->|calls|";
    lines.push(`    ${src} ${style} ${tgt}`);
  }

  if (involvedNodes.size === 0) {
    lines.push("    empty[No data flow or call edges found]");
  }

  return lines.join("\n");
}

/**
 * Generate a Mermaid sequence diagram from calls edges.
 *
 * Participants are ordered by first appearance in the graph traversal.
 */
export function generateSequence(graph: CytoscapeGraph): string {
  const lines: string[] = ["sequenceDiagram"];

  const labelOf = new Map<string, string>();
  for (const node of graph.elements.nodes) {
    labelOf.set(node.data.id, node.data.label);
  }

  // Collect calls edges
  const callEdges = graph.elements.edges.filter(
    (e) => e.data.kind === "calls" && !e.data._isMeta,
  );

  if (callEdges.length === 0) {
    lines.push("    Note over A: No call edges found");
    return lines.join("\n");
  }

  // Declare participants in order of first appearance
  const seen = new Set<string>();
  for (const edge of callEdges) {
    for (const nodeId of [edge.data.source, edge.data.target]) {
      if (!seen.has(nodeId)) {
        seen.add(nodeId);
        const label = labelOf.get(nodeId) ?? nodeId;
        lines.push(`    participant ${sanitizeId(nodeId)} as ${label}`);
      }
    }
  }

  // Render calls as messages
  for (const edge of callEdges) {
    const src = sanitizeId(edge.data.source);
    const tgt = sanitizeId(edge.data.target);
    lines.push(`    ${src}->>+${tgt}: calls`);
    lines.push(`    ${tgt}-->>-${src}: return`);
  }

  return lines.join("\n");
}

/**
 * Generate a Mermaid C4 component diagram.
 *
 * Maps system/service/component kinds to C4 constructs.
 */
export function generateC4(graph: CytoscapeGraph): string {
  const lines: string[] = ["C4Component"];

  const labelOf = new Map<string, string>();
  const kindOf = new Map<string, string>();
  for (const node of graph.elements.nodes) {
    labelOf.set(node.data.id, node.data.label);
    kindOf.set(node.data.id, node.data.kind);
  }

  // Build parent-children map
  const children = new Map<string, string[]>();
  const rootNodes: string[] = [];

  for (const node of graph.elements.nodes) {
    if (node.data.parent) {
      const siblings = children.get(node.data.parent) ?? [];
      siblings.push(node.data.id);
      children.set(node.data.parent, siblings);
    } else {
      rootNodes.push(node.data.id);
    }
  }

  function renderC4Node(nodeId: string, indent: string): void {
    const kids = children.get(nodeId);
    const label = labelOf.get(nodeId) ?? nodeId;
    const kind = kindOf.get(nodeId) ?? "component";

    if (kids && kids.length > 0) {
      const boundary = kind === "system" ? "System_Boundary" :
                       kind === "service" ? "Container_Boundary" : "Boundary";
      lines.push(`${indent}${boundary}(${sanitizeId(nodeId)}, "${label}") {`);
      for (const child of kids) {
        renderC4Node(child, indent + "    ");
      }
      lines.push(`${indent}}`);
    } else {
      const c4Type = kind === "system" ? "System" :
                     kind === "service" ? "Container" : "Component";
      lines.push(`${indent}${c4Type}(${sanitizeId(nodeId)}, "${label}")`);
    }
  }

  for (const root of rootNodes) {
    renderC4Node(root, "    ");
  }

  // Render relationships
  for (const edge of graph.elements.edges) {
    if (edge.data.kind === "contains") continue;
    if (edge.data._isMeta) continue;
    const src = sanitizeId(edge.data.source);
    const tgt = sanitizeId(edge.data.target);
    lines.push(`    Rel(${src}, ${tgt}, "${edge.data.kind}")`);
  }

  return lines.join("\n");
}
