/** Diagram types available in the Mermaid drawer. */
export type DiagramType = "flowchart" | "dataflow" | "sequence" | "c4";

/** Reactive store for Mermaid drawer state. */
class MermaidStore {
  open = $state(false);
  diagramType = $state<DiagramType>("flowchart");
  source = $state("");

  /** Toggle the drawer open/closed. */
  toggle() {
    this.open = !this.open;
  }

  /** Close the drawer. */
  close() {
    this.open = false;
  }
}

export const mermaidStore = new MermaidStore();
