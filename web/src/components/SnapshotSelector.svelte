<script lang="ts">
  import type { Snapshot, Version } from "../lib/types";

  interface Props {
    snapshots: Snapshot[];
    selectedVersion: Version | null;
    onselect: (version: Version) => void;
  }

  let { snapshots, selectedVersion, onselect }: Props = $props();

  function handleChange(event: Event) {
    const target = event.target as HTMLSelectElement;
    const version = parseInt(target.value, 10);
    if (!isNaN(version)) {
      onselect(version);
    }
  }
</script>

<div class="snapshot-selector">
  <label for="snapshot-select">Snapshot:</label>
  <select id="snapshot-select" value={selectedVersion ?? ""} onchange={handleChange}>
    <option value="" disabled>Select a version...</option>
    {#each snapshots as snap}
      <option value={snap.version}>
        v{snap.version} ({snap.kind}{snap.commit_ref ? ` - ${snap.commit_ref}` : ""})
      </option>
    {/each}
  </select>
</div>

<style>
  .snapshot-selector {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  label {
    color: var(--text-muted);
    font-size: 0.85rem;
    white-space: nowrap;
  }

  select {
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--border);
    padding: 0.3rem 0.5rem;
    border-radius: 4px;
    font-size: 0.85rem;
  }
</style>
