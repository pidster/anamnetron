<script lang="ts">
  import type { ApiNode, ApiEdge } from "../lib/types";
  import { graphStore } from "../stores/graph.svelte";
  import { selectionStore } from "../stores/selection.svelte";

  interface Props {
    node: ApiNode | null;
    children: ApiNode[];
    ancestors: ApiNode[];
    dependencies: ApiEdge[];
    dependents: ApiEdge[];
    loading: boolean;
  }

  let { node, children, ancestors, dependencies, dependents, loading }: Props = $props();

  /** Build a lookup from node ID to canonical path from the graph data. */
  const nodePathById = $derived.by(() => {
    const map = new Map<string, string>();
    const nodes = graphStore.graph?.elements?.nodes;
    if (nodes) {
      for (const n of nodes) {
        map.set(n.data.id, n.data.canonical_path);
      }
    }
    return map;
  });

  /** Resolve a node ID to its canonical path, falling back to the raw ID. */
  function resolvePath(id: string): string {
    return nodePathById.get(id) ?? id;
  }

  function selectNode(nodeId: string) {
    selectionStore.selectedNodeId = nodeId;
    selectionStore.panelOpen = true;
  }

  function close() {
    selectionStore.clear();
  }
</script>

{#if node}
  <aside class="node-detail">
    <header>
      <h2>{node.name}</h2>
      <button onclick={close} aria-label="Close panel">&times;</button>
    </header>

    <section>
      <dl>
        <dt>Kind</dt>
        <dd>{node.kind} / {node.sub_kind}</dd>
        <dt>Path</dt>
        <dd><code>{node.canonical_path}</code></dd>
        {#if node.qualified_name}
          <dt>Qualified Name</dt>
          <dd><code>{node.qualified_name}</code></dd>
        {/if}
        {#if node.language}
          <dt>Language</dt>
          <dd>{node.language}</dd>
        {/if}
        {#if node.source_ref}
          <dt>Source</dt>
          <dd><code>{node.source_ref}</code></dd>
        {/if}
        <dt>Provenance</dt>
        <dd>{node.provenance}</dd>
        {#if node.metadata && Object.keys(node.metadata).length > 0}
          {#each Object.entries(node.metadata) as [key, value]}
            <dt class="metric-key">{key}</dt>
            <dd class="metric-value">{value}</dd>
          {/each}
        {/if}
      </dl>
    </section>

    {#if loading}
      <p class="loading">Loading details...</p>
    {:else}
      {#if ancestors.length > 0}
        <section>
          <h3>Ancestors ({ancestors.length})</h3>
          <ul>
            {#each ancestors as a}
              <li>
                <button class="link-btn" onclick={() => selectNode(a.id)}>
                  <code>{a.canonical_path}</code>
                </button>
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      {#if children.length > 0}
        <section>
          <h3>Children ({children.length})</h3>
          <ul>
            {#each children as c}
              <li>
                <button class="link-btn" onclick={() => selectNode(c.id)}>
                  <code>{c.canonical_path}</code>
                </button>
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      {#if dependencies.length > 0}
        <section>
          <h3>Dependencies ({dependencies.length})</h3>
          <ul>
            {#each dependencies as d}
              <li>
                <button class="link-btn" onclick={() => selectNode(d.target)}>
                  {d.kind}: <code>{resolvePath(d.target)}</code>
                </button>
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      {#if dependents.length > 0}
        <section>
          <h3>Dependents ({dependents.length})</h3>
          <ul>
            {#each dependents as d}
              <li>
                <button class="link-btn" onclick={() => selectNode(d.source)}>
                  {d.kind}: <code>{resolvePath(d.source)}</code>
                </button>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    {/if}
  </aside>
{/if}

<style>
  .node-detail {
    width: 360px;
    flex-shrink: 0;
    background: var(--surface);
    border-left: 1px solid var(--border);
    overflow-y: auto;
    padding: 1rem;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  header h2 {
    font-size: 1.1rem;
    word-break: break-all;
  }

  header button {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 1.5rem;
    cursor: pointer;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.25rem 0.75rem;
  }

  dt {
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  dd {
    font-size: 0.85rem;
    word-break: break-all;
  }

  section {
    margin-bottom: 1rem;
  }

  h3 {
    font-size: 0.9rem;
    color: var(--text-muted);
    margin-bottom: 0.5rem;
  }

  ul {
    list-style: none;
    font-size: 0.85rem;
  }

  li {
    padding: 0.2rem 0;
    border-bottom: 1px solid var(--border);
  }

  code {
    font-size: 0.8rem;
    color: var(--accent);
  }

  .link-btn {
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    text-align: left;
    font: inherit;
    color: inherit;
  }

  .link-btn:hover code {
    text-decoration: underline;
  }

  .metric-key {
    color: var(--text-muted);
    font-size: 0.8rem;
    font-style: italic;
  }

  .metric-value {
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
  }

  .loading {
    color: var(--text-muted);
    font-style: italic;
  }
</style>
