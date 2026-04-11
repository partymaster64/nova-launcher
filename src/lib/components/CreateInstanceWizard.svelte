<script>
  import { invoke } from '@tauri-apps/api/core'
  import { createEventDispatcher } from 'svelte'
  import { manifest, config, isOnline } from '../../store.js'
  import { loaderIcon } from '../loaderIcons.js'
  import { t } from '../i18n.js'
  import Select from './Select.svelte'
  import Checkbox from './Checkbox.svelte'

  const dispatch = createEventDispatcher()

  let step = 1
  let name = ''
  let selectedVersion = ''
  let showSnapshots = false
  let selectedLoader = 'vanilla'
  let loaderVersion = ''
  let loaderVersions = []
  let loadingLoaderVersions = false
  let creating = false
  let error = null
  let installStep = ''
  let installPercent = 0
  let installDone = false
  let installError = null
  let pollIv = null

  let mf = null
  manifest.subscribe(v => {
    mf = v
    if (v && v.versions && !selectedVersion) {
      const firstRelease = v.versions.find(ver => ver.type === 'release')
      if (firstRelease) selectedVersion = firstRelease.id
    }
  })

  $: filteredVersions = (mf?.versions ?? []).filter(v =>
    v.type === 'release' || (showSnapshots && v.type === 'snapshot')
  )

  $: loaders = [
    { id: 'vanilla',  label: 'Vanilla',   color: '#34d399', note: null,                          recommended: false, wip: false,
      logo: `<svg viewBox="0 0 24 24" fill="currentColor"><polygon points="12,3 21,7.5 12,12 3,7.5" opacity=".9"/><polygon points="3,7.5 12,12 12,21 3,16.5" opacity=".55"/><polygon points="21,7.5 12,12 12,21 21,16.5" opacity=".72"/></svg>` },
    { id: 'fabric',   label: 'Fabric',    color: '#dba96e', note: null,                          recommended: true,  wip: false },
    { id: 'quilt',    label: 'Quilt',     color: '#60a5fa', note: $t('createInstance.fabricFork'), recommended: false, wip: false },
    { id: 'forge',    label: 'Forge',     color: '#c084fc', note: null,                          recommended: false, wip: false },
    { id: 'neoforge', label: 'NeoForge',  color: '#f97316', note: $t('createInstance.forgeFork'),  recommended: false, wip: false },
  ]

  async function fetchLoaderVersions() {
    if (selectedLoader === 'vanilla') {
      loaderVersions = []
      loaderVersion = ''
      return
    }
    if (!selectedVersion) return
    loadingLoaderVersions = true
    try {
      loaderVersions = await invoke('get_loader_versions', { loader: selectedLoader, mcVersion: selectedVersion })
      loaderVersion = loaderVersions[0] || ''
    } catch (e) {
      loaderVersions = []
      loaderVersion = ''
    } finally {
      loadingLoaderVersions = false
    }
  }

  function selectLoader(id) {
    selectedLoader = id
    loaderVersions = []
    loaderVersion = ''
    fetchLoaderVersions()
  }

  function goNext() {
    if (step === 1) {
      if (!selectedVersion) return
      step = 2
      // Pre-fetch loader versions for non-vanilla loaders
      if (selectedLoader !== 'vanilla') fetchLoaderVersions()
    }
  }

  function goBack() {
    if (step === 2) step = 1
  }

  async function create() {
    step = 3
    creating = true
    error = null
    installDone = false
    installError = null
    installStep = $t('createInstance.titleCreating')
    installPercent = 0
    try {
      const updated = await invoke('create_instance', {
        name: name.trim(),
        mcVersion: selectedVersion,
        loader: selectedLoader,
        loaderVersion: loaderVersion || null,
        iconUrl: null,
      })
      config.set(updated)
      const instanceId = updated.active_instance_id
      await invoke('prepare_instance', { instanceId })
      pollIv = setInterval(async () => {
        try {
          const p = await invoke('get_install_progress', { instanceId })
          if (!p) return
          installStep = p.step
          installPercent = p.percent
          if (p.done) {
            clearInterval(pollIv)
            pollIv = null
            if (p.error) {
              installError = p.error
            } else {
              installDone = true
              setTimeout(() => dispatch('close'), 1800)
            }
          }
        } catch (_) {}
      }, 500)
    } catch (e) {
      installError = e?.toString() || 'Unbekannter Fehler'
      creating = false
    }
  }

  function backdropClick(e) {
    if (e.target === e.currentTarget) dispatch('close')
  }
</script>

<!-- svelte-ignore a11y-click-events-have-key-events -->
<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="overlay" on:click={backdropClick}>
  <div class="modal" role="dialog" aria-modal="true">
    <!-- Header -->
    <div class="modal-header">
      <div class="modal-title">
        {#if step === 1}{$t('createInstance.titleName')}
        {:else if step === 2}{$t('createInstance.titleLoader')}
        {:else}{$t('createInstance.titleCreating')}
        {/if}
      </div>
      <div class="step-indicator">
        <span class="step" class:active={step >= 1} class:done={step > 1}>1</span>
        <div class="step-line" class:done={step > 1}></div>
        <span class="step" class:active={step >= 2} class:done={step > 2}>2</span>
        <div class="step-line" class:done={step > 2}></div>
        <span class="step" class:active={step >= 3}>3</span>
      </div>
    </div>

    <!-- Offline-Hinweis -->
    {#if !$isOnline}
      <div class="modal-body offline-body">
        <div class="offline-msg">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="32" height="32"><line x1="1" y1="1" x2="23" y2="23"/><path d="M16.72 11.06A10.94 10.94 0 0 1 19 12.55M5 12.55a10.94 10.94 0 0 1 5.17-2.39M10.71 5.05A16 16 0 0 1 22.56 9M1.42 9a15.91 15.91 0 0 1 4.7-2.88M8.53 16.11a6 6 0 0 1 6.95 0M12 20h.01"/></svg>
          <span class="offline-msg-title">Kein Internet</span>
          <span class="offline-msg-sub">Instanzen können nur mit Internetverbindung erstellt werden.</span>
        </div>
        <div class="modal-footer">
          <button class="btn btn-ghost" on:click={() => dispatch('close')}>{$t('common.cancel')}</button>
        </div>
      </div>
    <!-- Step 1: Basic settings -->
    {:else if step === 1}
      <div class="modal-body">
        <div class="form-group">
          <label class="form-label" for="inst-name">{$t('createInstance.name')}</label>
          <input
            id="inst-name"
            class="input"
            type="text"
            placeholder={$t('createInstance.namePlaceholder')}
            bind:value={name}
          />
        </div>

        <div class="form-group">
          <div class="version-label-row">
            <span class="form-label">{$t('createInstance.version')}</span>
            <Checkbox bind:checked={showSnapshots}>{$t('createInstance.showSnapshots')}</Checkbox>
          </div>
          <Select
            bind:value={selectedVersion}
            options={filteredVersions.map(v => ({ value: v.id, label: v.id + (v.type !== 'release' ? ' (' + v.type + ')' : '') }))}
            placeholder={$t('createInstance.versionPlaceholder')}
            searchable={true}
          />
          {#if !mf}
            <p class="hint">{$t('createInstance.versionsLoading')}</p>
          {/if}
        </div>
      </div>

      <div class="modal-footer">
        <button class="btn btn-ghost" on:click={() => dispatch('close')}>{$t('common.cancel')}</button>
        <button class="btn btn-primary" on:click={goNext} disabled={!selectedVersion}>
          {$t('common.next')}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="14" height="14"><line x1="5" y1="12" x2="19" y2="12"/><polyline points="12 5 19 12 12 19"/></svg>
        </button>
      </div>

    <!-- Step 2: Loader selection -->
    {:else if step === 2}
      <div class="modal-body">
        <div class="loader-grid">
          {#each loaders as l}
            <button
              class="loader-card"
              class:selected={selectedLoader === l.id}
              class:wip={l.wip}
              on:click={() => !l.wip && selectLoader(l.id)}
              style="--loader-color: {l.color}"
              disabled={l.wip}
            >
              <div class="loader-icon" style="background: {l.color}22; border-color: {l.color}44; color: {l.color}">
                {@html loaderIcon(l.id) || l.logo}
              </div>
              <div class="loader-info">
                <span class="loader-name">{l.label}</span>
                {#if l.note}
                  <span class="loader-note">{l.note}</span>
                {/if}
              </div>
              {#if l.wip}
                <span class="wip-badge">{$t('common.soon')}</span>
              {:else if l.recommended}
                <span class="rec-badge">{$t('common.recommended')}</span>
              {/if}
            </button>
          {/each}
        </div>

        {#if selectedLoader !== 'vanilla'}
          <div class="form-group" style="margin-top: 16px">
            <label class="form-label" for="loader-version">{$t('createInstance.loaderVersion')}</label>
            {#if loadingLoaderVersions}
              <div class="hint">{$t('createInstance.loaderVersionsLoading')}</div>
            {:else if loaderVersions.length > 0}
              <Select bind:value={loaderVersion} options={loaderVersions} placeholder={$t('createInstance.versionPlaceholder')} />
            {:else}
              <div class="hint">{$t('createInstance.loaderVersionsEmpty')}</div>
            {/if}
          </div>
        {/if}
      </div>

      <div class="modal-footer">
        <button class="btn btn-ghost" on:click={goBack}>{$t('common.back')}</button>
        <button class="btn btn-primary" on:click={create}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="14" height="14"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
          {$t('common.create')}
        </button>
      </div>

    <!-- Step 3: Installing -->
    {:else}
      <div class="modal-body creating-state">
        {#if installDone}
          <div class="done-state">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="44" height="44" style="color: var(--success)"><circle cx="12" cy="12" r="10"/><polyline points="8 12 11 15 16 9"/></svg>
            <p class="done-text">Instanz bereit!</p>
          </div>
        {:else if installError}
          <div class="error-state">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="40" height="40" style="color: var(--error)"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
            <p class="error-text">{installError}</p>
            <button class="btn btn-ghost" on:click={() => { step = 2; installError = null; creating = false }}>{$t('common.back')}</button>
          </div>
        {:else}
          <div class="install-progress">
            <div class="wiz-logo-wrap">
              <svg viewBox="0 0 160 160" xmlns="http://www.w3.org/2000/svg"
                style="--ni-1:color-mix(in srgb,var(--accent) 18%,black);--ni-2:color-mix(in srgb,var(--accent) 32%,black);--ni-3:color-mix(in srgb,var(--accent) 52%,black);--ni-4:var(--accent);--ni-5:color-mix(in srgb,var(--accent) 60%,white);--ni-6:color-mix(in srgb,var(--accent) 35%,white);--ni-7:color-mix(in srgb,var(--accent) 14%,white);"
              >
                <rect class="wr wr1" x="4"  y="4"  width="152" height="152" rx="18" fill="var(--ni-1)"/>
                <rect class="wr wr2" x="16" y="16" width="128" height="128" rx="16" fill="var(--ni-2)"/>
                <rect class="wr wr3" x="28" y="28" width="104" height="104" rx="14" fill="var(--ni-3)"/>
                <rect class="wr wr4" x="40" y="40" width="80"  height="80"  rx="12" fill="var(--ni-4)"/>
                <rect class="wr wr5" x="52" y="52" width="56"  height="56"  rx="10" fill="var(--ni-5)"/>
                <rect class="wr wr6" x="64" y="64" width="32"  height="32"  rx="7"  fill="var(--ni-6)"/>
                <rect class="wr wr7" x="73" y="73" width="14"  height="14"  rx="4"  fill="var(--ni-7)"/>
              </svg>
            </div>
            <div class="install-step">{installStep || $t('createInstance.titleCreating')}</div>
            <div class="progress-bar-wrap">
              <div class="progress-bar-fill" style="width: {Math.round(installPercent * 100)}%"></div>
            </div>
            <div class="progress-pct">{Math.round(installPercent * 100)}%</div>
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.65);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    animation: fadeIn 0.15s ease;
  }

  @keyframes fadeIn {
    from { opacity: 0 }
    to   { opacity: 1 }
  }

  .modal {
    background: var(--surface2);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    width: 100%;
    max-width: 520px;
    margin: 16px;
    display: flex;
    flex-direction: column;
    animation: slideIn 0.18s ease;
    overflow: hidden;
  }

  @keyframes slideIn {
    from { transform: translateY(-12px); opacity: 0 }
    to   { transform: translateY(0);     opacity: 1 }
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 22px 28px 18px;
    border-bottom: 1px solid var(--border);
  }

  .modal-title {
    font-size: 16px;
    font-weight: 700;
    color: var(--text);
  }

  .step-indicator {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .step {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    font-weight: 700;
    background: var(--surface3);
    color: var(--text-muted);
    border: 1px solid var(--border);
    transition: all 0.2s ease;
  }

  .step.active {
    background: rgba(var(--accent-rgb), 0.2);
    color: var(--accent);
    border-color: var(--accent);
  }

  .step.done {
    background: var(--accent);
    color: #fff;
    border-color: var(--accent);
  }

  .step-line {
    width: 20px;
    height: 1px;
    background: var(--border);
    transition: background 0.2s ease;
  }

  .step-line.done {
    background: var(--accent);
  }

  .modal-body {
    padding: 24px 28px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-height: 60vh;
    overflow-y: auto;
  }
  .offline-body { max-height: none; }
  .offline-msg { display: flex; flex-direction: column; align-items: center; gap: 10px; padding: 16px 0; color: #fb923c; text-align: center; }
  .offline-msg-title { font-size: 14px; font-weight: 700; color: #fb923c; }
  .offline-msg-sub { font-size: 12px; color: var(--text-muted); }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    padding: 16px 28px 22px;
    border-top: 1px solid var(--border);
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .form-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .version-label-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }


  .hint {
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }

  /* Loader grid */
  .loader-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .loader-card {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 14px;
    border-radius: var(--radius);
    background: var(--surface);
    border: 1px solid var(--border);
    cursor: pointer;
    transition: all 0.15s ease;
    position: relative;
    text-align: left;
  }

  .loader-card:hover {
    border-color: var(--loader-color);
    background: var(--surface3);
  }

  .loader-card.selected {
    border-color: var(--loader-color);
    background: color-mix(in srgb, var(--loader-color) 8%, var(--surface));
    box-shadow: 0 0 0 1px var(--loader-color);
  }

  .loader-icon {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    border: 1px solid;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    padding: 5px;
  }

  .loader-icon :global(svg) {
    width: 100%;
    height: 100%;
  }

  .loader-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .loader-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
  }

  .loader-note {
    font-size: 10px;
    color: var(--text-muted);
  }

  .loader-card.wip {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .loader-card:disabled {
    cursor: not-allowed;
  }

  .rec-badge {
    position: absolute;
    top: 6px;
    right: 6px;
    font-size: 9px;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 100px;
    background: rgba(52, 211, 153, 0.15);
    color: var(--success);
    border: 1px solid rgba(52, 211, 153, 0.3);
    white-space: nowrap;
  }

  .wip-badge {
    position: absolute;
    top: 6px;
    right: 6px;
    font-size: 9px;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 100px;
    background: rgba(148, 163, 184, 0.15);
    color: var(--text-muted);
    border: 1px solid rgba(148, 163, 184, 0.25);
    white-space: nowrap;
  }

  /* Step 3: install progress */
  .creating-state {
    align-items: center;
    justify-content: center;
    padding: 32px 28px;
    gap: 16px;
    min-height: 280px;
    color: var(--text-dim);
  }

  .install-progress {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 10px;
    align-items: center;
  }

  .wiz-logo-wrap {
    width: 96px; height: 96px;
    display: flex; align-items: center; justify-content: center;
    margin-bottom: 8px;
    animation: wizFloat 3s ease-in-out 1.2s infinite;
  }
  .wiz-logo-wrap svg { width: 96px; height: 96px; }

  @keyframes wizFloat {
    0%, 100% { transform: translateY(0); }
    50%       { transform: translateY(-5px); }
  }

  .wr {
    transform-box: fill-box; transform-origin: center;
    animation: wizBuild 0.45s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  .wr7 { animation-delay: 0.00s; } .wr6 { animation-delay: 0.10s; }
  .wr5 { animation-delay: 0.20s; } .wr4 { animation-delay: 0.30s; }
  .wr3 { animation-delay: 0.40s; } .wr2 { animation-delay: 0.50s; }
  .wr1 { animation-delay: 0.60s; }

  @keyframes wizBuild {
    0%   { opacity: 0; transform: scale(0.2); }
    70%  { opacity: 1; transform: scale(1.08); }
    100% { opacity: 1; transform: scale(1); }
  }

  .install-step {
    font-size: 13px;
    color: var(--text-dim);
    text-align: center;
    min-height: 20px;
  }

  .progress-bar-wrap {
    width: 100%;
    max-width: 320px;
    height: 6px;
    background: var(--surface3);
    border-radius: 100px;
    overflow: hidden;
  }

  .progress-bar-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 100px;
    transition: width 0.3s ease;
  }

  .progress-pct {
    font-size: 11px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .done-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    animation: fadeIn 0.25s ease;
  }

  .done-text {
    font-size: 14px;
    font-weight: 600;
    color: var(--success);
  }

  .error-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    text-align: center;
  }

  .error-text {
    color: var(--error);
    font-size: 13px;
  }
</style>
