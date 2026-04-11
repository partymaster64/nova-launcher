<script>
  import { invoke } from '@tauri-apps/api/core'
  import { onMount } from 'svelte'
  import { crashEvent, currentPage, detailInstanceId, detailActiveTab } from '../../store.js'
  import { t } from '../i18n.js'

  export let instanceId
  export let instanceName
  export let error

  let analysis = null
  let loading = true
  // Last ERROR/FATAL lines extracted for context
  let errorLines = []

  onMount(async () => {
    try {
      const [a, logLines] = await Promise.all([
        invoke('analyze_crash_log', { instanceId }),
        invoke('get_instance_logs', { instanceId }).catch(() => []),
      ])
      analysis = a
      errorLines = logLines
        .filter(l => /\/(error|fatal)\]|exception in thread|caused by:/i.test(l))
        .slice(-6)
    } catch (_) {}
    loading = false
  })

  function close() {
    invoke('clear_instance_error', { instanceId }).catch(() => {})
    crashEvent.set(null)
  }

  function openLogs() {
    detailInstanceId.set(instanceId)
    detailActiveTab.set('logs')
    currentPage.set('instance-detail')
    close()
  }
</script>

<!-- svelte-ignore a11y-click-events-have-key-events -->
<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="overlay" on:click|self={close}>
  <div class="modal">
    <div class="modal-header">
      <div class="header-icon">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="20" height="20">
          <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
          <line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
        </svg>
      </div>
      <div class="header-text">
        <div class="header-title">{$t('crash.title')}</div>
        <div class="header-sub">{instanceName}</div>
      </div>
      <button class="close-btn" on:click={close}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
          <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </button>
    </div>

    <!-- Raw error -->
    <div class="error-box">
      <div class="error-box-label">{$t('crash.errorLabel')}</div>
      <pre class="error-text">{error}</pre>
    </div>

    <!-- Structured analysis -->
    {#if loading}
      <div class="analysis-loading">
        <div class="spinner"></div>
        <span>{$t('crash.analyzing')}</span>
      </div>
    {:else if analysis?.has_issues}
      <div class="issues-section">
        <div class="issues-label">{$t('crash.suggestionsLabel')}</div>
        {#each analysis.issues as issue}
          <div class="issue" class:issue-warn={issue.severity === 'warning'}>
            <div class="issue-title">{$t(`crash.issues.${issue.code}.title`)}</div>
            <div class="issue-desc">{$t(`crash.issues.${issue.code}.description`)}</div>
            <div class="issue-fix">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="11" height="11"><polyline points="9 11 12 14 22 4"/><path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11"/></svg>
              {$t(`crash.issues.${issue.code}.suggestion`)}
            </div>
          </div>
        {/each}
      </div>
    {/if}

    <!-- Last error lines from log -->
    {#if errorLines.length > 0}
      <details class="log-details">
        <summary>{$t('crash.logContext')}</summary>
        <div class="log-snippet">
          {#each errorLines as line}
            <div class="log-line">{line}</div>
          {/each}
        </div>
      </details>
    {/if}

    <div class="modal-actions">
      <button class="btn btn-ghost" on:click={openLogs}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/></svg>
        {$t('crash.openLogs')}
      </button>
      <button class="btn btn-primary" on:click={close}>{$t('common.close')}</button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed; inset: 0; z-index: 9000;
    background: rgba(0,0,0,0.65);
    display: flex; align-items: center; justify-content: center;
    padding: 24px;
    animation: fadeIn 0.15s ease;
  }
  @keyframes fadeIn { from { opacity: 0 } to { opacity: 1 } }

  .modal {
    background: var(--surface);
    border: 1px solid rgba(239,68,68,0.4);
    border-radius: var(--radius-lg);
    box-shadow: 0 24px 64px rgba(0,0,0,0.6), 0 0 0 1px rgba(239,68,68,0.15);
    width: 100%; max-width: 520px;
    display: flex; flex-direction: column; gap: 0;
    animation: slideUp 0.18s ease;
    max-height: 85vh; overflow: hidden;
  }
  @keyframes slideUp { from { transform: translateY(12px); opacity: 0 } to { transform: translateY(0); opacity: 1 } }

  .modal-header {
    display: flex; align-items: center; gap: 12px;
    padding: 18px 20px 16px;
    border-bottom: 1px solid rgba(239,68,68,0.2);
    flex-shrink: 0;
  }
  .header-icon {
    width: 40px; height: 40px; border-radius: 10px; flex-shrink: 0;
    background: rgba(239,68,68,0.15);
    border: 1px solid rgba(239,68,68,0.35);
    display: flex; align-items: center; justify-content: center;
    color: #ef4444;
  }
  .header-text { flex: 1; min-width: 0; }
  .header-title { font-size: 15px; font-weight: 800; color: var(--text); }
  .header-sub   { font-size: 12px; color: var(--text-muted); margin-top: 1px; }
  .close-btn {
    width: 30px; height: 30px; border-radius: 50%;
    display: flex; align-items: center; justify-content: center;
    background: var(--surface2); border: 1px solid var(--border);
    color: var(--text-muted); cursor: pointer; flex-shrink: 0;
    transition: all 0.12s;
  }
  .close-btn:hover { background: var(--surface3); color: var(--text); }

  /* Scrollable body */
  .error-box, .issues-section, .log-details { padding: 0 20px; }
  .modal > * + * { margin-top: 0; }
  .error-box { padding-top: 16px; padding-bottom: 4px; }
  .issues-section { padding-top: 4px; padding-bottom: 4px; }
  .log-details { padding-top: 4px; padding-bottom: 4px; }

  /* Overflow scroll on the inner content area */
  .modal { overflow-y: auto; }
  .modal-header { position: sticky; top: 0; background: var(--surface); z-index: 1; }
  .modal-actions { position: sticky; bottom: 0; background: var(--surface); z-index: 1; }

  .error-box-label {
    font-size: 10.5px; font-weight: 700; letter-spacing: 0.06em;
    text-transform: uppercase; color: var(--text-muted); margin-bottom: 6px;
  }
  .error-text {
    font-family: monospace; font-size: 11.5px;
    color: #ef4444; line-height: 1.55;
    background: rgba(239,68,68,0.07);
    border: 1px solid rgba(239,68,68,0.2);
    border-radius: var(--radius-sm);
    padding: 10px 12px; margin: 0;
    white-space: pre-wrap; word-break: break-all;
    max-height: 120px; overflow-y: auto;
  }

  .analysis-loading {
    display: flex; align-items: center; gap: 10px;
    padding: 14px 20px; font-size: 12.5px; color: var(--text-muted);
  }
  .spinner {
    width: 16px; height: 16px; border-radius: 50%; flex-shrink: 0;
    border: 2px solid rgba(var(--accent-rgb),0.2); border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg) } }

  .issues-label {
    font-size: 10.5px; font-weight: 700; letter-spacing: 0.06em;
    text-transform: uppercase; color: var(--text-muted);
    margin-bottom: 8px; margin-top: 12px;
  }
  .issue {
    background: rgba(239,68,68,0.07);
    border: 1px solid rgba(239,68,68,0.25);
    border-left: 3px solid #ef4444;
    border-radius: calc(var(--radius) - 2px);
    padding: 10px 12px;
    margin-bottom: 7px;
  }
  .issue-warn {
    background: rgba(245,158,11,0.07);
    border-color: rgba(245,158,11,0.25);
    border-left-color: #f59e0b;
  }
  .issue-title { font-size: 12px; font-weight: 700; color: var(--text); margin-bottom: 3px; }
  .issue-desc  { font-size: 11.5px; color: var(--text-dim); line-height: 1.45; margin-bottom: 5px; }
  .issue-fix {
    font-size: 11px; color: var(--accent);
    display: flex; align-items: flex-start; gap: 5px; line-height: 1.45;
  }
  .issue-fix svg { flex-shrink: 0; margin-top: 1px; }

  .log-details {
    margin-top: 8px;
  }
  .log-details summary {
    font-size: 11.5px; color: var(--text-muted); cursor: pointer;
    padding: 4px 0; user-select: none;
  }
  .log-details summary:hover { color: var(--text); }
  .log-snippet {
    margin-top: 8px;
    background: var(--surface2); border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 8px 10px; font-family: monospace; font-size: 10.5px;
    color: #ef4444; line-height: 1.55; max-height: 140px; overflow-y: auto;
  }
  .log-line { white-space: pre-wrap; word-break: break-all; }

  .modal-actions {
    display: flex; justify-content: flex-end; gap: 8px;
    padding: 14px 20px 18px;
    border-top: 1px solid var(--border);
    margin-top: 12px;
  }
</style>
