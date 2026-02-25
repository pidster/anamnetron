/**
 * State that can be encoded in the URL hash.
 *
 * URL format: #<view>:<focus_path>&<params>
 * Examples:
 *   #treemap:/aeon/wal&v=1&node=abc
 *   #bundle:/aeon/rest/handlers&v=3
 *   #matrix:&v=1          (no focus, full graph)
 */
export interface HashState {
  view?: string;
  focusPath?: string;
  version?: number;
  node?: string;
  diff?: number;
  mermaid?: string;
}

/** Parse the URL hash into state. */
export function parseHash(hash: string): HashState {
  const clean = hash.replace(/^#/, "");
  if (!clean) return {};

  const state: HashState = {};

  // New format: <view>:<path>&<params>
  const ampIdx = clean.indexOf("&");
  const prefix = ampIdx >= 0 ? clean.slice(0, ampIdx) : clean;
  const paramStr = ampIdx >= 0 ? clean.slice(ampIdx + 1) : "";

  const colonIdx = prefix.indexOf(":");
  if (colonIdx >= 0) {
    // New format detected
    const view = prefix.slice(0, colonIdx);
    const path = prefix.slice(colonIdx + 1);
    if (view) state.view = view;
    if (path) state.focusPath = decodeURIComponent(path);

    const params = new URLSearchParams(paramStr);
    const v = params.get("v");
    if (v) state.version = parseInt(v, 10);

    const node = params.get("node");
    if (node) state.node = node;

    const diff = params.get("diff");
    if (diff) state.diff = parseInt(diff, 10);

    const mermaid = params.get("mermaid");
    if (mermaid) state.mermaid = mermaid;
  } else {
    // Legacy format: key=value params only (backwards compatibility)
    const params = new URLSearchParams(clean);

    const v = params.get("v");
    if (v) state.version = parseInt(v, 10);

    const node = params.get("node");
    if (node) state.node = node;

    const diff = params.get("diff");
    if (diff) state.diff = parseInt(diff, 10);

    const scope = params.get("scope");
    if (scope) state.focusPath = scope;

    const mermaid = params.get("mermaid");
    if (mermaid) state.mermaid = mermaid;

    const view = params.get("view");
    if (view) state.view = view;
  }

  return state;
}

/** Build a URL hash string from state. */
export function buildHash(state: HashState): string {
  const view = state.view ?? "";
  const focusPath = state.focusPath ?? "";

  const params = new URLSearchParams();
  if (state.version !== undefined) params.set("v", String(state.version));
  if (state.node !== undefined) params.set("node", state.node);
  if (state.diff !== undefined) params.set("diff", String(state.diff));
  if (state.mermaid !== undefined) params.set("mermaid", state.mermaid);

  const paramStr = params.toString();

  // Build: #<view>:<focusPath>&<params>
  const prefix = `${view}:${encodeURIComponent(focusPath).replace(/%2F/gi, "/")}`;

  if (paramStr) {
    return `#${prefix}&${paramStr}`;
  }
  // Only emit hash if there's meaningful content
  if (view || focusPath) {
    return `#${prefix}`;
  }
  return "";
}
