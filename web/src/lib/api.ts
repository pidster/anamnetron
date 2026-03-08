import type {
  Project,
  Snapshot,
  ApiNode,
  ApiEdge,
  CytoscapeGraph,
  ConformanceReport,
  SnapshotDiff,
  Version,
} from "./types";

const BASE = "";

async function fetchJson<T>(url: string): Promise<T> {
  const response = await fetch(url);
  if (!response.ok) {
    const body = await response.json().catch(() => ({ error: response.statusText }));
    throw new Error(body.error || `HTTP ${response.status}`);
  }
  return response.json();
}

/** GET /api/health */
export function getHealth(): Promise<{ status: string }> {
  return fetchJson(`${BASE}/api/health`);
}

/** GET /api/snapshots */
export function getSnapshots(): Promise<Snapshot[]> {
  return fetchJson(`${BASE}/api/snapshots`);
}

/** GET /api/snapshots/{v}/nodes */
export function getNodes(version: Version): Promise<ApiNode[]> {
  return fetchJson(`${BASE}/api/snapshots/${version}/nodes`);
}

/** GET /api/snapshots/{v}/nodes/{id} */
export function getNode(version: Version, id: string): Promise<ApiNode> {
  return fetchJson(`${BASE}/api/snapshots/${version}/nodes/${encodeURIComponent(id)}`);
}

/** GET /api/snapshots/{v}/nodes/{id}/children */
export function getChildren(version: Version, id: string): Promise<ApiNode[]> {
  return fetchJson(`${BASE}/api/snapshots/${version}/nodes/${encodeURIComponent(id)}/children`);
}

/** GET /api/snapshots/{v}/nodes/{id}/ancestors */
export function getAncestors(version: Version, id: string): Promise<ApiNode[]> {
  return fetchJson(`${BASE}/api/snapshots/${version}/nodes/${encodeURIComponent(id)}/ancestors`);
}

/** GET /api/snapshots/{v}/nodes/{id}/dependencies */
export function getDependencies(version: Version, id: string): Promise<ApiEdge[]> {
  return fetchJson(`${BASE}/api/snapshots/${version}/nodes/${encodeURIComponent(id)}/dependencies`);
}

/** GET /api/snapshots/{v}/nodes/{id}/dependents */
export function getDependents(version: Version, id: string): Promise<ApiEdge[]> {
  return fetchJson(`${BASE}/api/snapshots/${version}/nodes/${encodeURIComponent(id)}/dependents`);
}

/** GET /api/snapshots/{v}/edges */
export function getEdges(version: Version, kind?: string): Promise<ApiEdge[]> {
  const params = kind ? `?kind=${encodeURIComponent(kind)}` : "";
  return fetchJson(`${BASE}/api/snapshots/${version}/edges${params}`);
}

/** GET /api/snapshots/{v}/graph */
export function getGraph(version: Version): Promise<CytoscapeGraph> {
  return fetchJson(`${BASE}/api/snapshots/${version}/graph`);
}

/** GET /api/conformance/design/{v} */
export function getDesignConformance(version: Version): Promise<ConformanceReport> {
  return fetchJson(`${BASE}/api/conformance/design/${version}`);
}

/** GET /api/conformance?design=V&analysis=V */
export function getConformance(design: Version, analysis: Version): Promise<ConformanceReport> {
  return fetchJson(`${BASE}/api/conformance?design=${design}&analysis=${analysis}`);
}

/** GET /api/search?path=GLOB&version=V */
export function searchNodes(path: string, version: Version): Promise<ApiNode[]> {
  return fetchJson(`${BASE}/api/search?path=${encodeURIComponent(path)}&version=${version}`);
}

/** GET /api/diff?from=V1&to=V2 */
export function getDiff(from: Version, to: Version): Promise<SnapshotDiff> {
  return fetchJson(`${BASE}/api/diff?from=${from}&to=${to}`);
}

/** GET /api/projects */
export function getProjects(): Promise<Project[]> {
  return fetchJson(`${BASE}/api/projects`);
}

/** GET /api/projects/{project}/snapshots */
export function getProjectSnapshots(project: string): Promise<Snapshot[]> {
  return fetchJson(`${BASE}/api/projects/${encodeURIComponent(project)}/snapshots`);
}

/** GET /api/projects/{project}/snapshots/{v}/graph */
export function getProjectGraph(project: string, version: Version): Promise<CytoscapeGraph> {
  return fetchJson(`${BASE}/api/projects/${encodeURIComponent(project)}/snapshots/${version}/graph`);
}

/** GET /api/projects/{project}/snapshots/{v}/nodes */
export function getProjectNodes(project: string, version: Version): Promise<ApiNode[]> {
  return fetchJson(`${BASE}/api/projects/${encodeURIComponent(project)}/snapshots/${version}/nodes`);
}

/** GET /api/projects/{project}/snapshots/{v}/edges */
export function getProjectEdges(project: string, version: Version, kind?: string): Promise<ApiEdge[]> {
  const params = kind ? `?kind=${encodeURIComponent(kind)}` : "";
  return fetchJson(`${BASE}/api/projects/${encodeURIComponent(project)}/snapshots/${version}/edges${params}`);
}

/** Root entry returned by the roots API. */
export interface RootEntry {
  node_id: string;
  canonical_path: string;
  name: string;
}

/** Root analysis result. */
export interface RootAnalysis {
  call_tree_roots: RootEntry[];
  dependency_sources: RootEntry[];
  dependency_sinks: RootEntry[];
  containment_roots: RootEntry[];
  leaf_sinks: RootEntry[];
}

/** GET /api/projects/{project}/snapshots/{v}/roots */
export function getProjectRoots(project: string, version: Version): Promise<RootAnalysis> {
  return fetchJson(`${BASE}/api/projects/${encodeURIComponent(project)}/snapshots/${version}/roots`);
}
