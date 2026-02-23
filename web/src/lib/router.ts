/** State that can be encoded in the URL hash. */
export interface HashState {
  version?: number;
  node?: string;
  diff?: number;
  scope?: string;
  mermaid?: string;
  view?: string;
}

/** Parse the URL hash into state. */
export function parseHash(hash: string): HashState {
  const clean = hash.replace(/^#/, "");
  if (!clean) return {};

  const params = new URLSearchParams(clean);
  const state: HashState = {};

  const v = params.get("v");
  if (v) state.version = parseInt(v, 10);

  const node = params.get("node");
  if (node) state.node = node;

  const diff = params.get("diff");
  if (diff) state.diff = parseInt(diff, 10);

  const scope = params.get("scope");
  if (scope) state.scope = scope;

  const mermaid = params.get("mermaid");
  if (mermaid) state.mermaid = mermaid;

  const view = params.get("view");
  if (view) state.view = view;

  return state;
}

/** Build a URL hash string from state. */
export function buildHash(state: HashState): string {
  const params = new URLSearchParams();
  if (state.version !== undefined) params.set("v", String(state.version));
  if (state.node !== undefined) params.set("node", state.node);
  if (state.diff !== undefined) params.set("diff", String(state.diff));
  if (state.scope !== undefined) params.set("scope", state.scope);
  if (state.mermaid !== undefined) params.set("mermaid", state.mermaid);
  if (state.view !== undefined) params.set("view", state.view);
  const str = params.toString();
  return str ? `#${str}` : "";
}
