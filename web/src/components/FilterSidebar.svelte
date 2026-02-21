<script lang="ts">
  import { filterStore } from "../stores/filter.svelte";
  import { formatLabel } from "../lib/filter-logic";

  const NODE_KINDS = ["system", "service", "component", "unit"] as const;
  const EDGE_KINDS = ["depends", "calls", "implements", "extends", "data_flow", "exports"] as const;
</script>

{#if filterStore.sidebarOpen}
  <aside class="filter-sidebar">
    <div class="sidebar-header">
      <h3>Filters</h3>
      <button class="reset-btn" onclick={() => filterStore.resetAll()}>Reset</button>
    </div>

    <div class="filter-sections">
      <section class="filter-section">
        <h4>Node Kind</h4>
        {#each NODE_KINDS as kind}
          <label class="filter-item">
            <input
              type="checkbox"
              checked={filterStore.nodeKinds.has(kind)}
              onchange={() => filterStore.toggleNodeKind(kind)}
            />
            {formatLabel(kind)}
          </label>
        {/each}
      </section>

      <section class="filter-section">
        <h4>Edge Kind</h4>
        {#each EDGE_KINDS as kind}
          <label class="filter-item">
            <input
              type="checkbox"
              checked={filterStore.edgeKinds.has(kind)}
              onchange={() => filterStore.toggleEdgeKind(kind)}
            />
            {formatLabel(kind)}
          </label>
        {/each}
      </section>

      {#if filterStore.availableSubKinds.length > 0}
        <section class="filter-section">
          <h4>Sub-Kind</h4>
          {#each filterStore.availableSubKinds as subKind}
            <label class="filter-item">
              <input
                type="checkbox"
                checked={filterStore.subKinds.has(subKind)}
                onchange={() => filterStore.toggleSubKind(subKind)}
              />
              {formatLabel(subKind)}
            </label>
          {/each}
        </section>
      {/if}

      {#if filterStore.availableLanguages.length > 0}
        <section class="filter-section">
          <h4>Language</h4>
          {#each filterStore.availableLanguages as lang}
            <label class="filter-item">
              <input
                type="checkbox"
                checked={filterStore.languages.has(lang)}
                onchange={() => filterStore.toggleLanguage(lang)}
              />
              {formatLabel(lang)}
            </label>
          {/each}
        </section>
      {/if}
    </div>
  </aside>
{/if}

<style>
  .filter-sidebar {
    width: 220px;
    min-width: 220px;
    background: var(--surface);
    border-right: 1px solid var(--border);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }

  .sidebar-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--border);
  }

  .sidebar-header h3 {
    margin: 0;
    font-size: 0.9rem;
    color: var(--text);
  }

  .reset-btn {
    background: var(--bg);
    color: var(--text-muted);
    border: 1px solid var(--border);
    font-size: 0.75rem;
    padding: 0.15rem 0.4rem;
    border-radius: 3px;
    cursor: pointer;
  }

  .filter-sections {
    padding: 0.5rem 0;
    overflow-y: auto;
  }

  .filter-section {
    padding: 0 0.75rem 0.5rem;
  }

  .filter-section h4 {
    margin: 0.5rem 0 0.25rem;
    font-size: 0.8rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .filter-item {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.15rem 0;
    font-size: 0.82rem;
    color: var(--text);
    cursor: pointer;
  }

  .filter-item input[type="checkbox"] {
    margin: 0;
    accent-color: var(--accent);
  }
</style>
