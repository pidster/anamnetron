<script lang="ts">
  import type { Project } from "../lib/types";

  interface Props {
    projects: Project[];
    selectedProject: string | null;
    onselect: (projectId: string) => void;
  }

  let { projects, selectedProject, onselect }: Props = $props();

  function handleChange(event: Event) {
    const target = event.target as HTMLSelectElement;
    if (target.value) {
      onselect(target.value);
    }
  }
</script>

{#if projects.length > 1}
  <div class="project-selector">
    <label for="project-select">Project:</label>
    <select id="project-select" value={selectedProject ?? ""} onchange={handleChange}>
      <option value="" disabled>Select a project...</option>
      {#each projects as project}
        <option value={project.id}>
          {project.name}
        </option>
      {/each}
    </select>
  </div>
{/if}

<style>
  .project-selector {
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
