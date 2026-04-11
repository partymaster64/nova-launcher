<script>
  import { toasts } from '../../store.js'
</script>

<div class="toast-container">
  {#each $toasts as t (t.id)}
    <div class="toast toast-{t.type}">
      {#if t.type === 'error'}
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16" style="flex-shrink:0"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
      {:else if t.type === 'success'}
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="16" height="16" style="flex-shrink:0"><polyline points="20 6 9 17 4 12"/></svg>
      {:else if t.type === 'info'}
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16" style="flex-shrink:0"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
      {/if}
      <span>{t.message}</span>
    </div>
  {/each}
</div>

<style>
  .toast-container {
    position: fixed;
    bottom: 20px;
    right: 20px;
    z-index: 9999;
    display: flex;
    flex-direction: column;
    gap: 8px;
    pointer-events: none;
  }

  .toast {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 10px 14px;
    border-radius: var(--radius);
    font-size: 13px;
    line-height: 1.4;
    max-width: 360px;
    word-break: break-word;
    animation: slideIn 0.2s ease;
    box-shadow: 0 4px 20px rgba(0,0,0,0.4);
  }

  .toast-error   { background: var(--surface); border: 1px solid rgba(248,113,113,0.4); color: var(--error); }
  .toast-success { background: var(--surface); border: 1px solid rgba(52,211,153,0.4);  color: var(--success); }
  .toast-info    { background: var(--surface); border: 1px solid rgba(96,165,250,0.4);  color: var(--info); }

  @keyframes slideIn {
    from { opacity: 0; transform: translateY(8px); }
    to   { opacity: 1; transform: translateY(0); }
  }
</style>
