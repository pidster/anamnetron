/** State that can be encoded in the URL hash. */
export interface HashState {
  version?: number;
  node?: string;
  layout?: string;
  diff?: number;
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

  const layout = params.get("layout");
  if (layout) state.layout = layout;

  const diff = params.get("diff");
  if (diff) state.diff = parseInt(diff, 10);

  return state;
}

/** Build a URL hash string from state. */
export function buildHash(state: HashState): string {
  const params = new URLSearchParams();
  if (state.version !== undefined) params.set("v", String(state.version));
  if (state.node !== undefined) params.set("node", state.node);
  if (state.layout !== undefined) params.set("layout", state.layout);
  if (state.diff !== undefined) params.set("diff", String(state.diff));
  const str = params.toString();
  return str ? `#${str}` : "";
}
