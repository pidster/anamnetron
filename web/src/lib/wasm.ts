/**
 * TypeScript wrapper around the svt-wasm WASM module.
 *
 * Provides a typed API that mirrors the HTTP API patterns from `api.ts`,
 * converting raw JSON string responses into typed objects and handling
 * bigint version conversion internally.
 */
import type { ApiNode, ApiEdge, Version } from "./types";
import init, { WasmStore } from "../../../crates/wasm/pkg/svt_wasm";

/** Edge direction for node-scoped edge queries. */
export type EdgeDirection = "outgoing" | "incoming" | "both";

/** Cached singleton state for lazy initialization. */
let cachedStore: WasmGraphStore | null = null;
let initPromise: Promise<WasmGraphStore | null> | null = null;

/**
 * Typed wrapper around the raw WASM `WasmStore`.
 *
 * All version parameters accept regular `number` values (the project's
 * `Version` type) and are converted to `bigint` internally. JSON string
 * responses from the WASM layer are parsed into the corresponding
 * TypeScript types from `types.ts`.
 */
export class WasmGraphStore {
  private readonly store: WasmStore;

  constructor(store: WasmStore) {
    this.store = store;
  }

  /**
   * Load a snapshot from arrays of nodes and edges.
   *
   * @param nodes - Array of nodes to load
   * @param edges - Array of edges to load
   * @param projectId - Optional project ID (defaults to `"default"`)
   * @returns The new snapshot version number
   */
  loadSnapshot(
    nodes: ApiNode[],
    edges: ApiEdge[],
    projectId?: string,
  ): Version {
    const version = this.store.load_snapshot(
      JSON.stringify(nodes),
      JSON.stringify(edges),
      projectId ?? undefined,
    );
    return Number(version);
  }

  /** Get a node by its ID within a version. Returns `null` if not found. */
  getNode(version: Version, id: string): ApiNode | null {
    const json = this.store.get_node(BigInt(version), id);
    return JSON.parse(json) as ApiNode | null;
  }

  /** Get a node by its canonical path within a version. Returns `null` if not found. */
  getNodeByPath(version: Version, path: string): ApiNode | null {
    const json = this.store.get_node_by_path(BigInt(version), path);
    return JSON.parse(json) as ApiNode | null;
  }

  /** Get all nodes for a version. */
  getAllNodes(version: Version): ApiNode[] {
    const json = this.store.get_all_nodes(BigInt(version));
    return JSON.parse(json) as ApiNode[];
  }

  /** Get the direct children of a node. */
  getChildren(version: Version, nodeId: string): ApiNode[] {
    const json = this.store.get_children(BigInt(version), nodeId);
    return JSON.parse(json) as ApiNode[];
  }

  /** Get the parent of a node. Returns `null` if the node has no parent. */
  getParent(version: Version, nodeId: string): ApiNode | null {
    const json = this.store.get_parent(BigInt(version), nodeId);
    return JSON.parse(json) as ApiNode | null;
  }

  /** Get all ancestors of a node (parent, grandparent, etc.). */
  getAncestors(version: Version, nodeId: string): ApiNode[] {
    const json = this.store.get_ancestors(BigInt(version), nodeId);
    return JSON.parse(json) as ApiNode[];
  }

  /** Get all descendants of a node. */
  getDescendants(version: Version, nodeId: string): ApiNode[] {
    const json = this.store.get_descendants(BigInt(version), nodeId);
    return JSON.parse(json) as ApiNode[];
  }

  /**
   * Get edges connected to a node.
   *
   * @param version - Snapshot version
   * @param nodeId - Node ID to query edges for
   * @param direction - `"outgoing"`, `"incoming"`, or `"both"`
   * @param kind - Optional edge kind filter (e.g. `"depends"`, `"contains"`)
   */
  getEdges(
    version: Version,
    nodeId: string,
    direction: EdgeDirection,
    kind?: string,
  ): ApiEdge[] {
    const json = this.store.get_edges(
      BigInt(version),
      nodeId,
      direction,
      kind ?? null,
    );
    return JSON.parse(json) as ApiEdge[];
  }

  /**
   * Get all edges for a version, optionally filtered by kind.
   *
   * @param version - Snapshot version
   * @param kind - Optional edge kind filter
   */
  getAllEdges(version: Version, kind?: string): ApiEdge[] {
    const json = this.store.get_all_edges(BigInt(version), kind ?? null);
    return JSON.parse(json) as ApiEdge[];
  }

  /**
   * Get dependencies of a node (nodes it depends on).
   *
   * @param version - Snapshot version
   * @param nodeId - Node ID to query
   * @param transitive - If `true`, follows the dependency chain recursively
   */
  getDependencies(
    version: Version,
    nodeId: string,
    transitive: boolean = false,
  ): ApiNode[] {
    const json = this.store.get_dependencies(
      BigInt(version),
      nodeId,
      transitive,
    );
    return JSON.parse(json) as ApiNode[];
  }

  /**
   * Get dependents of a node (nodes that depend on it).
   *
   * @param version - Snapshot version
   * @param nodeId - Node ID to query
   * @param transitive - If `true`, follows the dependent chain recursively
   */
  getDependents(
    version: Version,
    nodeId: string,
    transitive: boolean = false,
  ): ApiNode[] {
    const json = this.store.get_dependents(
      BigInt(version),
      nodeId,
      transitive,
    );
    return JSON.parse(json) as ApiNode[];
  }

  /**
   * Search for nodes whose canonical path matches a glob pattern.
   *
   * @param version - Snapshot version
   * @param pattern - Glob pattern to match against canonical paths
   */
  search(version: Version, pattern: string): ApiNode[] {
    const json = this.store.search(BigInt(version), pattern);
    return JSON.parse(json) as ApiNode[];
  }
}

/**
 * Initialize the WASM module and create a `WasmGraphStore` instance.
 *
 * The result is cached so that subsequent calls return the same store
 * without re-initializing the WASM module. If initialization fails,
 * a warning is logged and `null` is returned.
 */
export async function initWasm(): Promise<WasmGraphStore | null> {
  if (cachedStore) {
    return cachedStore;
  }

  // Deduplicate concurrent initialization calls
  if (initPromise) {
    return initPromise;
  }

  initPromise = (async () => {
    try {
      await init();
      const raw = new WasmStore();
      cachedStore = new WasmGraphStore(raw);
      return cachedStore;
    } catch (err) {
      console.warn("Failed to initialize WASM store:", err);
      return null;
    } finally {
      initPromise = null;
    }
  })();

  return initPromise;
}

/**
 * Get the cached `WasmGraphStore` instance, or `null` if WASM has not
 * been initialized yet (call `initWasm()` first).
 */
export function getWasmStore(): WasmGraphStore | null {
  return cachedStore;
}
