/**
 * State that can be encoded in the URL hash.
 *
 * URL format: #<view>=<path>&<params>
 * Examples:
 *   #treemap=/aeon/wal&v=1
 *   #bundle=/aeon/rest/handlers&v=3
 *   #matrix=&v=1              (no path, full graph)
 *   #treemap=/aeon/wal        (no extra params)
 *
 * The path is the canonical path of the current location (focused or selected node).
 */
export interface HashState {
  view?: string;
  /** Canonical path of the current location (focus or selection). */
  path?: string;
  version?: number;
  diff?: number;
  mermaid?: string;
  project?: string;
}

/** Known view names for detecting the new URL format. */
const VIEW_NAMES = new Set(["treemap", "bundle", "matrix", "chord", "sunburst", "mermaid"]);

/** Parse the URL hash into state. */
export function parseHash(hash: string): HashState {
  const clean = hash.replace(/^#/, "");
  if (!clean) return {};

  const state: HashState = {};

  // Split prefix from params at first &
  const ampIdx = clean.indexOf("&");
  const prefix = ampIdx >= 0 ? clean.slice(0, ampIdx) : clean;
  const paramStr = ampIdx >= 0 ? clean.slice(ampIdx + 1) : "";

  // New format: <view>=<path> where view is a known name
  const eqIdx = prefix.indexOf("=");
  const possibleView = eqIdx >= 0 ? prefix.slice(0, eqIdx) : "";

  if (eqIdx >= 0 && VIEW_NAMES.has(possibleView)) {
    // New format detected
    const path = prefix.slice(eqIdx + 1);
    state.view = possibleView;
    if (path) state.path = decodeURIComponent(path);

    const params = new URLSearchParams(paramStr);
    const v = params.get("v");
    if (v) state.version = parseInt(v, 10);

    const diff = params.get("diff");
    if (diff) state.diff = parseInt(diff, 10);

    const mermaid = params.get("mermaid");
    if (mermaid) state.mermaid = mermaid;

    const p = params.get("p");
    if (p) state.project = p;
  } else {
    // Legacy format: key=value params only (backwards compatibility)
    const params = new URLSearchParams(clean);

    const v = params.get("v");
    if (v) state.version = parseInt(v, 10);

    // Legacy: scope or node become path
    const scope = params.get("scope");
    const node = params.get("node");
    if (scope) state.path = scope;
    else if (node) state.path = node;

    const diff = params.get("diff");
    if (diff) state.diff = parseInt(diff, 10);

    const mermaid = params.get("mermaid");
    if (mermaid) state.mermaid = mermaid;

    const view = params.get("view");
    if (view) state.view = view;

    const p = params.get("p");
    if (p) state.project = p;
  }

  return state;
}

/** Build a URL hash string from state. */
export function buildHash(state: HashState): string {
  const view = state.view ?? "";
  const path = state.path ?? "";

  const params = new URLSearchParams();
  if (state.version !== undefined) params.set("v", String(state.version));
  if (state.diff !== undefined) params.set("diff", String(state.diff));
  if (state.mermaid !== undefined) params.set("mermaid", state.mermaid);
  if (state.project !== undefined) params.set("p", state.project);

  const paramStr = params.toString();

  // Keep slashes readable in the path
  const encodedPath = encodeURIComponent(path).replace(/%2F/gi, "/");
  const prefix = `${view}=${encodedPath}`;

  if (paramStr) {
    return `#${prefix}&${paramStr}`;
  }
  if (view || path) {
    return `#${prefix}`;
  }
  return "";
}
