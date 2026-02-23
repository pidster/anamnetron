import { describe, it, expect } from "vitest";
import { generateFlowchart, generateDataFlow, generateSequence, generateC4 } from "../mermaid-gen";
import type { CytoscapeGraph } from "../types";

function makeGraph(
  nodes: Array<{ id: string; label: string; kind?: string; parent?: string }>,
  edges: Array<{ id: string; source: string; target: string; kind: string; _isMeta?: boolean }> = [],
): CytoscapeGraph {
  return {
    elements: {
      nodes: nodes.map((n) => ({
        data: {
          id: n.id,
          label: n.label,
          kind: n.kind ?? "component",
          sub_kind: "",
          canonical_path: `/${n.id}`,
          parent: n.parent,
        },
      })),
      edges: edges.map((e) => ({
        data: {
          id: e.id,
          source: e.source,
          target: e.target,
          kind: e.kind,
          _isMeta: e._isMeta,
          _count: e._isMeta ? 2 : undefined,
        },
      })),
    },
  };
}

describe("generateFlowchart", () => {
  it("produces a flowchart with nodes and edges", () => {
    const graph = makeGraph(
      [
        { id: "a", label: "Service A" },
        { id: "b", label: "Service B" },
      ],
      [{ id: "e1", source: "a", target: "b", kind: "depends" }],
    );

    const result = generateFlowchart(graph);
    expect(result).toContain("flowchart TD");
    expect(result).toContain('a["Service A"]');
    expect(result).toContain('b["Service B"]');
    expect(result).toContain("a -->|depends| b");
  });

  it("renders parent nodes as subgraphs", () => {
    const graph = makeGraph([
      { id: "sys", label: "System" },
      { id: "svc", label: "Service", parent: "sys" },
    ]);

    const result = generateFlowchart(graph);
    expect(result).toContain('subgraph sys["System"]');
    expect(result).toContain('svc["Service"]');
    expect(result).toContain("end");
  });

  it("skips contains edges", () => {
    const graph = makeGraph(
      [
        { id: "a", label: "A" },
        { id: "b", label: "B", parent: "a" },
      ],
      [{ id: "e1", source: "a", target: "b", kind: "contains" }],
    );

    const result = generateFlowchart(graph);
    expect(result).not.toContain("-->|contains|");
  });

  it("skips meta-edges", () => {
    const graph = makeGraph(
      [
        { id: "a", label: "A" },
        { id: "b", label: "B" },
      ],
      [{ id: "e1", source: "a", target: "b", kind: "depends", _isMeta: true }],
    );

    const result = generateFlowchart(graph);
    expect(result).not.toContain("-->|depends|");
  });

  it("sanitizes special characters in IDs", () => {
    const graph = makeGraph([{ id: "/svt/core", label: "Core" }]);
    const result = generateFlowchart(graph);
    expect(result).toContain("_svt_core");
    expect(result).not.toContain("/svt/core[");
  });
});

describe("generateDataFlow", () => {
  it("includes only data_flow and calls edges", () => {
    const graph = makeGraph(
      [
        { id: "a", label: "A" },
        { id: "b", label: "B" },
        { id: "c", label: "C" },
      ],
      [
        { id: "e1", source: "a", target: "b", kind: "data_flow" },
        { id: "e2", source: "b", target: "c", kind: "calls" },
        { id: "e3", source: "a", target: "c", kind: "depends" },
      ],
    );

    const result = generateDataFlow(graph);
    expect(result).toContain("flowchart LR");
    expect(result).toContain("data_flow");
    expect(result).toContain("calls");
    expect(result).not.toContain("depends");
  });

  it("only includes nodes involved in data flow", () => {
    const graph = makeGraph(
      [
        { id: "a", label: "A" },
        { id: "b", label: "B" },
        { id: "c", label: "Unused" },
      ],
      [{ id: "e1", source: "a", target: "b", kind: "data_flow" }],
    );

    const result = generateDataFlow(graph);
    expect(result).toContain('a["A"]');
    expect(result).toContain('b["B"]');
    expect(result).not.toContain("Unused");
  });

  it("shows empty message when no data flow edges exist", () => {
    const graph = makeGraph(
      [{ id: "a", label: "A" }],
      [{ id: "e1", source: "a", target: "a", kind: "depends" }],
    );

    const result = generateDataFlow(graph);
    expect(result).toContain("No data flow or call edges found");
  });
});

describe("generateSequence", () => {
  it("generates sequence diagram from calls edges", () => {
    const graph = makeGraph(
      [
        { id: "a", label: "Client" },
        { id: "b", label: "Server" },
      ],
      [{ id: "e1", source: "a", target: "b", kind: "calls" }],
    );

    const result = generateSequence(graph);
    expect(result).toContain("sequenceDiagram");
    expect(result).toContain("participant a as Client");
    expect(result).toContain("participant b as Server");
    expect(result).toContain("a->>+b: calls");
    expect(result).toContain("b-->>-a: return");
  });

  it("shows note when no call edges exist", () => {
    const graph = makeGraph(
      [{ id: "a", label: "A" }],
      [{ id: "e1", source: "a", target: "a", kind: "depends" }],
    );

    const result = generateSequence(graph);
    expect(result).toContain("No call edges found");
  });
});

describe("generateC4", () => {
  it("maps system kind to System_Boundary", () => {
    const graph = makeGraph([
      { id: "sys", label: "My System", kind: "system" },
      { id: "svc", label: "API Service", kind: "service", parent: "sys" },
    ]);

    const result = generateC4(graph);
    expect(result).toContain("C4Component");
    expect(result).toContain('System_Boundary(sys, "My System")');
    expect(result).toContain('Container(svc, "API Service")');
  });

  it("maps service kind to Container_Boundary when it has children", () => {
    const graph = makeGraph([
      { id: "svc", label: "Service", kind: "service" },
      { id: "comp", label: "Component", kind: "component", parent: "svc" },
    ]);

    const result = generateC4(graph);
    expect(result).toContain('Container_Boundary(svc, "Service")');
    expect(result).toContain('Component(comp, "Component")');
  });

  it("renders relationships", () => {
    const graph = makeGraph(
      [
        { id: "a", label: "A", kind: "component" },
        { id: "b", label: "B", kind: "component" },
      ],
      [{ id: "e1", source: "a", target: "b", kind: "depends" }],
    );

    const result = generateC4(graph);
    expect(result).toContain('Rel(a, b, "depends")');
  });
});
