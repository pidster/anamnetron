/** Snapshot version number. */
export type Version = number;

/** Snapshot kinds matching the server enum. */
export type SnapshotKind = "design" | "analysis" | "import";

/** Node kinds matching the server enum. */
export type NodeKind = "system" | "service" | "component" | "unit";

/** Edge kinds matching the server enum. */
export type EdgeKind =
  | "contains"
  | "depends"
  | "calls"
  | "implements"
  | "extends"
  | "data_flow"
  | "exports";

/** Provenance types. */
export type Provenance = "design" | "analysis" | "import" | "inferred";

/** Severity levels. */
export type Severity = "error" | "warning" | "info";

/** Constraint evaluation status. */
export type ConstraintStatus = "pass" | "fail" | "not_evaluable";

/** GET /api/snapshots response item. */
export interface Snapshot {
  version: Version;
  kind: SnapshotKind;
  commit_ref: string | null;
}

/** GET /api/snapshots/{v}/nodes response item (also from search). */
export interface ApiNode {
  id: string;
  canonical_path: string;
  qualified_name: string | null;
  kind: NodeKind;
  sub_kind: string;
  name: string;
  language: string | null;
  provenance: Provenance;
  source_ref: string | null;
  metadata: Record<string, unknown> | null;
}

/** GET /api/snapshots/{v}/edges response item. */
export interface ApiEdge {
  id: string;
  source: string;
  target: string;
  kind: EdgeKind;
  provenance: Provenance;
  metadata: Record<string, unknown> | null;
}

/** Cytoscape node data from /graph endpoint. */
export interface CyNodeData {
  id: string;
  label: string;
  kind: string;
  sub_kind: string;
  canonical_path: string;
  parent?: string;
  language?: string;
  source_ref?: string;
  /** Number of descendants when the node is in collapsed state. Set by expansion logic. */
  _childCount?: number;
}

/** Cytoscape edge data from /graph endpoint. */
export interface CyEdgeData {
  id: string;
  source: string;
  target: string;
  kind: string;
  /** True for aggregated meta-edges created by collapse logic. */
  _isMeta?: boolean;
  /** Number of real edges aggregated into this meta-edge. */
  _count?: number;
}

/** GET /api/snapshots/{v}/graph response. */
export interface CytoscapeGraph {
  elements: {
    nodes: Array<{ data: CyNodeData }>;
    edges: Array<{ data: CyEdgeData }>;
  };
}

/** Conformance violation. */
export interface Violation {
  source_path: string;
  target_path: string | null;
  edge_id: string | null;
  edge_kind: EdgeKind | null;
  source_ref: string | null;
}

/** Constraint evaluation result. */
export interface ConstraintResult {
  constraint_name: string;
  constraint_kind: string;
  status: ConstraintStatus;
  severity: Severity;
  message: string;
  violations: Violation[];
}

/** Unmatched node in conformance report. */
export interface UnmatchedNode {
  canonical_path: string;
  kind: NodeKind;
  name: string;
}

/** Conformance summary counts. */
export interface ConformanceSummary {
  passed: number;
  failed: number;
  warned: number;
  not_evaluable: number;
  unimplemented: number;
  undocumented: number;
}

/** GET /api/conformance response. */
export interface ConformanceReport {
  design_version: Version;
  analysis_version: Version | null;
  constraint_results: ConstraintResult[];
  unimplemented: UnmatchedNode[];
  undocumented: UnmatchedNode[];
  summary: ConformanceSummary;
}

/** API error response. */
export interface ApiError {
  error: string;
}

/** How a node or edge changed between snapshots. */
export type ChangeKind = "added" | "removed" | "changed";

/** A node that changed between two versions. */
export interface NodeChange {
  canonical_path: string;
  change: ChangeKind;
  kind: NodeKind;
  sub_kind: string;
  changed_fields: string[];
}

/** An edge that changed between two versions. */
export interface EdgeChange {
  source_path: string;
  target_path: string;
  edge_kind: EdgeKind;
  change: ChangeKind;
}

/** Summary counts for a snapshot diff. */
export interface DiffSummary {
  nodes_added: number;
  nodes_removed: number;
  nodes_changed: number;
  edges_added: number;
  edges_removed: number;
}

/** GET /api/diff response. */
export interface SnapshotDiff {
  from_version: Version;
  to_version: Version;
  node_changes: NodeChange[];
  edge_changes: EdgeChange[];
  summary: DiffSummary;
}
