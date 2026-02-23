/** Diagram types available in the Mermaid view. */
export type DiagramType = "flowchart" | "dataflow" | "sequence" | "c4";

/** Reactive store for Mermaid view state. */
class MermaidStore {
  diagramType = $state<DiagramType>("flowchart");
  source = $state("");
}

export const mermaidStore = new MermaidStore();
