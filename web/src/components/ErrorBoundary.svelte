<script lang="ts">
  interface Props {
    name: string;
    children: import("svelte").Snippet;
  }

  let { name, children }: Props = $props();
  let retryKey = $state(0);

  function retry(reset: () => void) {
    reset();
    retryKey++;
  }
</script>

{#key retryKey}
  <svelte:boundary>
    {@render children()}
    {#snippet failed(error, reset)}
      <div class="error-boundary">
        <h3 class="error-title">{name} failed</h3>
        <p class="error-message">{error instanceof Error ? error.message : String(error)}</p>
        <button class="retry-btn" onclick={() => retry(reset)}>Retry</button>
      </div>
    {/snippet}
  </svelte:boundary>
{/key}

<style>
  .error-boundary {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 2rem;
    flex: 1;
    min-height: 120px;
  }

  .error-title {
    color: var(--fail);
    margin: 0 0 0.5rem 0;
    font-size: 1rem;
  }

  .error-message {
    color: var(--text-muted);
    margin: 0 0 1rem 0;
    font-size: 0.85rem;
    max-width: 400px;
    text-align: center;
    word-break: break-word;
  }

  .retry-btn {
    background: var(--accent);
    color: #fff;
    border: none;
    padding: 0.4rem 1rem;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.85rem;
  }
</style>
