<script lang="ts">
  import type { ConformanceReport } from "../lib/types";

  interface Props {
    report: ConformanceReport;
    onclose: () => void;
  }

  let { report, onclose }: Props = $props();
</script>

<aside class="conformance-panel">
  <header>
    <h2>Conformance Report</h2>
    <button onclick={onclose} aria-label="Close">&times;</button>
  </header>

  <section class="summary">
    <div class="stat pass">{report.summary.passed} passed</div>
    <div class="stat fail">{report.summary.failed} failed</div>
    <div class="stat warn">{report.summary.warned} warned</div>
    <div class="stat muted">{report.summary.not_evaluable} n/a</div>
    <div class="stat unimpl">{report.summary.unimplemented} unimplemented</div>
    <div class="stat undoc">{report.summary.undocumented} undocumented</div>
  </section>

  {#if report.constraint_results.length > 0}
    <section>
      <h3>Constraints</h3>
      {#each report.constraint_results as cr}
        <div class="constraint" class:fail={cr.status === "fail"} class:pass={cr.status === "pass"}>
          <span class="badge">{cr.status}</span>
          <strong>{cr.constraint_name}</strong>
          <p>{cr.message}</p>
          {#if cr.violations.length > 0}
            <ul>
              {#each cr.violations as v}
                <li>
                  <code>{v.source_path}</code>
                  {#if v.target_path} &rarr; <code>{v.target_path}</code>{/if}
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {/each}
    </section>
  {/if}

  {#if report.unimplemented.length > 0}
    <section>
      <h3>Unimplemented ({report.unimplemented.length})</h3>
      <ul>
        {#each report.unimplemented as n}
          <li><code>{n.canonical_path}</code> ({n.kind})</li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if report.undocumented.length > 0}
    <section>
      <h3>Undocumented ({report.undocumented.length})</h3>
      <ul>
        {#each report.undocumented as n}
          <li><code>{n.canonical_path}</code> ({n.kind})</li>
        {/each}
      </ul>
    </section>
  {/if}
</aside>

<style>
  .conformance-panel {
    width: 400px;
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

  header button {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 1.5rem;
    cursor: pointer;
  }

  .summary {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.5rem;
    margin-bottom: 1rem;
  }

  .stat {
    padding: 0.5rem;
    border-radius: 4px;
    font-size: 0.85rem;
    text-align: center;
    background: var(--bg);
  }

  .stat.pass { border-left: 3px solid var(--pass); }
  .stat.fail { border-left: 3px solid var(--fail); }
  .stat.warn { border-left: 3px solid var(--warn); }
  .stat.muted { border-left: 3px solid var(--muted); }
  .stat.unimpl { border-left: 3px solid var(--warn); }
  .stat.undoc { border-left: 3px solid var(--muted); }

  .constraint {
    padding: 0.5rem;
    margin-bottom: 0.5rem;
    border-radius: 4px;
    background: var(--bg);
  }

  .constraint.fail { border-left: 3px solid var(--fail); }
  .constraint.pass { border-left: 3px solid var(--pass); }

  .badge {
    font-size: 0.75rem;
    text-transform: uppercase;
    padding: 0.1rem 0.3rem;
    border-radius: 2px;
    background: var(--surface);
  }

  h3 {
    font-size: 0.9rem;
    color: var(--text-muted);
    margin-bottom: 0.5rem;
  }

  section {
    margin-bottom: 1rem;
  }

  ul {
    list-style: none;
    font-size: 0.85rem;
  }

  li {
    padding: 0.2rem 0;
  }

  code {
    font-size: 0.8rem;
    color: var(--accent);
  }

  p {
    font-size: 0.85rem;
    color: var(--text-muted);
    margin: 0.25rem 0;
  }
</style>
