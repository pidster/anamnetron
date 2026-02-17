<script lang="ts">
  interface Props {
    onsearch: (query: string) => void;
  }

  let { onsearch }: Props = $props();
  let query = $state("");

  function handleSubmit(event: Event) {
    event.preventDefault();
    if (query.trim()) {
      onsearch(query.trim());
    }
  }
</script>

<form class="search-bar" onsubmit={handleSubmit}>
  <input
    type="text"
    placeholder="Search paths (e.g. /svt/core/**)"
    bind:value={query}
  />
  <button type="submit">Search</button>
</form>

<style>
  .search-bar {
    display: flex;
    gap: 0.5rem;
  }

  input {
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--border);
    padding: 0.3rem 0.5rem;
    border-radius: 4px;
    font-size: 0.85rem;
    flex: 1;
    min-width: 200px;
  }

  input::placeholder {
    color: var(--text-muted);
  }

  button {
    background: var(--accent);
    color: #fff;
    border: none;
    padding: 0.3rem 0.75rem;
    border-radius: 4px;
    font-size: 0.85rem;
    cursor: pointer;
  }

  button:hover {
    opacity: 0.9;
  }
</style>
