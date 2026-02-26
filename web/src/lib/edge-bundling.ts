import { cluster, type HierarchyPointNode } from "d3-hierarchy";
import type { TreeNode } from "./hierarchy";

/** Map from node ID to its root-to-leaf path (array of ancestor IDs). */
export type AncestorPathMap = Map<string, string[]>;

/** A bundled edge with control points for rendering as a radial curve. */
export interface BundledEdge {
  sourceId: string;
  targetId: string;
  kind: string;
  /** Sequence of [angle, radius] control points through the hierarchy. */
  points: Array<[number, number]>;
  /** Number of real edges aggregated into this edge (from _count on meta-edges, default 1). */
  count: number;
}

/**
 * Build a map from each leaf node ID to its root-to-leaf path.
 *
 * The path is an array of node IDs from root down to the leaf.
 */
export function buildAncestorPaths(
  root: HierarchyPointNode<TreeNode>,
): AncestorPathMap {
  const paths: AncestorPathMap = new Map();

  root.each((node) => {
    const path: string[] = [];
    let current: HierarchyPointNode<TreeNode> | null = node;
    while (current) {
      path.unshift(current.data.id);
      current = current.parent;
    }
    paths.set(node.data.id, path);
  });

  return paths;
}

/**
 * Find the path from source through their LCA to target.
 *
 * Returns the sequence of node IDs: source ancestors up to LCA,
 * then LCA descendants down to target.
 */
export function findPathThroughLCA(
  sourcePath: string[],
  targetPath: string[],
): string[] {
  // Find the LCA index (last shared ancestor)
  let lcaIdx = 0;
  const minLen = Math.min(sourcePath.length, targetPath.length);
  for (let i = 0; i < minLen; i++) {
    if (sourcePath[i] === targetPath[i]) {
      lcaIdx = i;
    } else {
      break;
    }
  }

  // Path: source -> ... -> LCA -> ... -> target
  // Reverse the source path segment (source up to LCA), then append target segment
  const sourceSegment = sourcePath.slice(lcaIdx, sourcePath.length).reverse();
  const targetSegment = targetPath.slice(lcaIdx + 1, targetPath.length);

  return [...sourceSegment, ...targetSegment];
}

/**
 * Compute bundled edges for all non-contains edges in the graph.
 *
 * Uses a radial cluster layout. Each edge's control points trace
 * the hierarchy path from source leaf → LCA → target leaf.
 * The actual beta-blending (tension) is handled by d3's `curveBundle`
 * in the rendering layer; this function only supplies the hierarchy
 * control points.
 */
export function computeBundledEdges(
  root: HierarchyPointNode<TreeNode>,
  edges: Array<{ data: { source: string; target: string; kind: string; _count?: number } }>,
): BundledEdge[] {
  const ancestorPaths = buildAncestorPaths(root);

  // Build a position map from node ID to [angle, radius] in radial coords
  const positionMap = new Map<string, [number, number]>();
  root.each((node) => {
    // d3 cluster with size([360, radius]) gives x=angle in degrees, y=radius
    const angleRad = (node.x * Math.PI) / 180;
    positionMap.set(node.data.id, [angleRad, node.y]);
  });

  const bundled: BundledEdge[] = [];

  for (const edge of edges) {
    const { source, target, kind } = edge.data;

    // Skip contains edges — they represent the hierarchy itself
    if (kind === "contains") continue;

    const sourcePath = ancestorPaths.get(source);
    const targetPath = ancestorPaths.get(target);

    // Skip edges with dangling endpoints (source or target not in tree)
    if (!sourcePath || !targetPath) continue;

    const lcaPath = findPathThroughLCA(sourcePath, targetPath);

    // Convert node IDs to radial coordinates
    const points: Array<[number, number]> = [];
    for (const nodeId of lcaPath) {
      const pos = positionMap.get(nodeId);
      if (pos) {
        points.push(pos);
      }
    }

    const count = (edge.data as Record<string, unknown>)._count as number | undefined;

    if (points.length >= 2) {
      bundled.push({ sourceId: source, targetId: target, kind, points, count: count ?? 1 });
    }
  }

  return bundled;
}

/**
 * Compute simple bezier arc edges for flat trees (root with only leaf children).
 *
 * Instead of routing through hierarchy control points (which produces no
 * bundling when there is only one level), this creates 3-point bezier arcs
 * where the control point is pulled inward toward the centre.
 */
export function computeArcEdges(
  root: HierarchyPointNode<TreeNode>,
  edges: Array<{ data: { source: string; target: string; kind: string; _count?: number } }>,
): BundledEdge[] {
  // Build a position map from node ID to [angle, radius] in radial coords
  const positionMap = new Map<string, [number, number]>();
  root.each((node) => {
    const angleRad = (node.x * Math.PI) / 180;
    positionMap.set(node.data.id, [angleRad, node.y]);
  });

  const arcs: BundledEdge[] = [];

  for (const edge of edges) {
    const { source, target, kind } = edge.data;
    if (kind === "contains") continue;

    const srcPos = positionMap.get(source);
    const tgtPos = positionMap.get(target);
    if (!srcPos || !tgtPos) continue;

    const [srcAngle, srcRadius] = srcPos;
    const [tgtAngle, tgtRadius] = tgtPos;

    // Control point at the midpoint angle, pulled inward to ~30% of the radius
    let midAngle = (srcAngle + tgtAngle) / 2;

    // If the arc spans more than PI radians, wrap around the other way
    let angleDiff = tgtAngle - srcAngle;
    if (angleDiff > Math.PI) angleDiff -= 2 * Math.PI;
    if (angleDiff < -Math.PI) angleDiff += 2 * Math.PI;
    midAngle = srcAngle + angleDiff / 2;

    const midRadius = Math.min(srcRadius, tgtRadius) * 0.3;

    const count = (edge.data as Record<string, unknown>)._count as number | undefined;

    arcs.push({
      sourceId: source,
      targetId: target,
      kind,
      points: [srcPos, [midAngle, midRadius], tgtPos],
      count: count ?? 1,
    });
  }

  return arcs;
}

/**
 * Create a radial cluster layout for the hierarchy.
 *
 * Returns the laid-out root with x (angle in degrees) and y (radius) on each node.
 *
 * Interior node radii are remapped so the root sits at `minRadiusFraction * innerRadius`
 * rather than at 0. Without this, edges whose LCA is near the root route through the
 * dead centre of the circle.
 *
 * A value of ~0.4 gives edges room to arc smoothly: shallow arcs for siblings,
 * deeper arcs for cross-module edges, conveying hierarchical distance visually.
 */
export function createRadialCluster(
  root: HierarchyPointNode<TreeNode>,
  innerRadius: number,
  minRadiusFraction = 0.4,
): HierarchyPointNode<TreeNode> {
  const layout = cluster<TreeNode>().size([360, innerRadius]);
  const result = layout(root);

  // Remap radii from [0, innerRadius] to [minRadius, innerRadius].
  // Leaves stay at innerRadius; the root moves to minRadius.
  const minRadius = innerRadius * minRadiusFraction;
  const scale = innerRadius > 0 ? (innerRadius - minRadius) / innerRadius : 1;
  result.each((node) => {
    node.y = minRadius + node.y * scale;
  });

  return result;
}
