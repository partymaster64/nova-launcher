<script>
  import { invoke } from '@tauri-apps/api/core'
  import { config, currentPage, modrinthResults } from '../../store.js'
  import Select from '../components/Select.svelte'

  let cfg = null
  config.subscribe(v => (cfg = v))

  // Pre-fill filters from active instance
  $: activeInst = cfg?.instances?.find(i => i.id === cfg?.active_instance_id)
  $: instName = activeInst?.name ?? 'Instanz'
  $: gameVersion = activeInst?.version ?? ''
  $: instLoader = activeInst?.loader ?? 'vanilla'

  let query = ''
  let filterVersion = ''
  let filterLoader = ''
  let results = null
  let loading = false
  let loadingMore = false
  let error = null
  let offset = 0
  let totalHits = 0

  // Toast
  let toast = null
  let toastTimeout = null

  function showToast(msg, type = 'success') {
    toast = { msg, type }
    if (toastTimeout) clearTimeout(toastTimeout)
    toastTimeout = setTimeout(() => (toast = null), 3000)
  }

  // Installing state: set of version_ids being installed
  let installing = new Set()

  function goBack() {
    currentPage.set('instances')
  }

  async function search(reset = true) {
    if (reset) {
      offset = 0
      results = null
      error = null
    }
    loading = reset
    loadingMore = !reset

    try {
      const gv = filterVersion || gameVersion || null
      const l = filterLoader || (instLoader !== 'vanilla' ? instLoader : null)

      const data = await invoke('search_modrinth', {
        query,
        gameVersion: gv,
        loader: l,
        offset,
      })

      if (reset) {
        results = data.hits
      } else {
        results = [...(results ?? []), ...data.hits]
      }
      totalHits = Number(data.total_hits)
      offset = results.length
    } catch (e) {
      error = e?.toString() || get(t)('mods.searchError')
    } finally {
      loading = false
      loadingMore = false
    }
  }

  async function loadMore() {
    await search(false)
  }

  async function install(hit) {
    if (installing.has(hit.project_id)) return

    // Fetch compatible versions
    const gv = filterVersion || gameVersion || null
    const l = filterLoader || (instLoader !== 'vanilla' ? instLoader : null)

    try {
      installing = new Set([...installing, hit.project_id])
      const versions = await invoke('get_modrinth_versions', {
        projectId: hit.project_id,
        gameVersion: gv,
        loader: l,
      })

      if (!versions || versions.length === 0) {
        showToast(get(t)('mods.noCompatible'), 'error')
        return
      }

      const filename = await invoke('install_mod', {
        projectId: hit.project_id,
        versionId: versions[0].id,
      })
      showToast(get(t)('mods.installed', { filename }))
    } catch (e) {
      showToast(e?.toString() || get(t)('mods.installError'), 'error')
    } finally {
      installing = new Set([...installing].filter(id => id !== hit.project_id))
    }
  }

  function formatDownloads(n) {
    if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M'
    if (n >= 1_000) return (n / 1_000).toFixed(0) + 'K'
    return String(n)
  }

  // Trigger initial search on mount
  import { onMount } from 'svelte'
  import { get } from 'svelte/store'
  import { t } from '../i18n.js'
  onMount(() => {
    filterVersion = gameVersion
    filterLoader = instLoader !== 'vanilla' ? instLoader : ''
    search()
  })

  function handleKeydown(e) {
    if (e.key === 'Enter') search()
  }
</script>

{#if toast}
  <div class="toast" class:toast-error={toast.type === 'error'}>
    {toast.msg}
  </div>
{/if}

<div class="page">
  <div class="page-header">
    <div class="header-left">
      <button class="back-btn" on:click={goBack} title={$t('common.back')}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="16" height="16"><line x1="19" y1="12" x2="5" y2="12"/><polyline points="12 19 5 12 12 5"/></svg>
      </button>
      <div>
        <h1 class="page-title">{$t('mods.title')}</h1>
        <div class="page-subtitle text-dim">{$t('mods.forInstance', { instance: instName })}</div>
      </div>
    </div>
  </div>

  <!-- Search bar -->
  <div class="search-bar">
    <div class="search-input-wrap">
      <svg class="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
      <!-- svelte-ignore a11y-autofocus -->
      <input
        class="input search-input"
        type="text"
        placeholder={$t('mods.searchPlaceholder')}
        bind:value={query}
        on:keydown={handleKeydown}
        autofocus
      />
    </div>

    <div class="filter-select">
      <Select
        bind:value={filterVersion}
        options={[{ value: '', label: $t('discover.allVersions') }, ...(gameVersion ? [{ value: gameVersion, label: gameVersion }] : [])]}
      />
    </div>

    <div class="filter-select">
      <Select
        bind:value={filterLoader}
        options={[
          { value: '', label: $t('mods.allLoaders') },
          { value: 'fabric', label: 'Fabric' },
          { value: 'forge', label: 'Forge' },
          { value: 'neoforge', label: 'NeoForge' },
          { value: 'quilt', label: 'Quilt' },
          { value: 'paper', label: 'Paper' },
        ]}
      />
    </div>

    <button class="btn btn-primary" on:click={() => search()}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="13" height="13"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
      {$t('common.search')}
    </button>
  </div>

  <div class="page-body">
    {#if loading}
      <!-- Skeleton -->
      <div class="results-grid">
        {#each Array(6) as _, i}
          <div class="mod-card skeleton-card">
            <div class="skel skel-icon"></div>
            <div class="skel-body">
              <div class="skel skel-title"></div>
              <div class="skel skel-line"></div>
              <div class="skel skel-line short"></div>
            </div>
          </div>
        {/each}
      </div>

    {:else if error}
      <div class="empty-state">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="48" height="48" style="color:var(--error)"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
        <p class="text-dim">{error}</p>
        <button class="btn btn-ghost" on:click={() => search()}>{$t('common.retry')}</button>
      </div>

    {:else if results !== null && results.length === 0}
      <div class="empty-state">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="48" height="48" style="color:var(--text-muted)"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
        <p class="text-dim">{$t('mods.noMods')}</p>
      </div>

    {:else if results}
      <div class="results-header">
        <span class="text-muted" style="font-size:12px">{$t('mods.results', { count: totalHits.toLocaleString() })}</span>
      </div>
      <div class="results-grid">
        {#each results as hit (hit.project_id)}
          <div class="mod-card">
            <div class="mod-icon-wrap">
              {#if hit.icon_url}
                <img class="mod-icon" src={hit.icon_url} alt={hit.title} loading="lazy" />
              {:else}
                <div class="mod-icon-placeholder">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="22" height="22"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/><polyline points="3.27 6.96 12 12.01 20.73 6.96"/><line x1="12" y1="22.08" x2="12" y2="12"/></svg>
                </div>
              {/if}
            </div>

            <div class="mod-body">
              <div class="mod-title">{hit.title}</div>
              <div class="mod-desc text-dim">{hit.description}</div>

              <div class="mod-meta">
                <span class="meta-chip">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="10" height="10"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                  {formatDownloads(hit.downloads)}
                </span>
                {#each hit.categories.slice(0, 2) as cat}
                  <span class="category-tag">{cat}</span>
                {/each}
              </div>
            </div>

            <button
              class="install-btn"
              class:installing={installing.has(hit.project_id)}
              disabled={installing.has(hit.project_id)}
              on:click={() => install(hit)}
            >
              {#if installing.has(hit.project_id)}
                <div class="mini-spinner"></div>
              {:else}
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="13" height="13"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
              {/if}
              {installing.has(hit.project_id) ? $t('mods.installing') : $t('mods.install')}
            </button>
          </div>
        {/each}
      </div>

      {#if results.length < totalHits}
        <div class="load-more">
          <button class="btn btn-ghost" on:click={loadMore} disabled={loadingMore}>
            {#if loadingMore}
              <div class="mini-spinner"></div>
              {$t('common.loading')}
            {:else}
              {$t('mods.loadMore', { remaining: totalHits - results.length })}
            {/if}
          </button>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .header-left {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .back-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: var(--radius-sm);
    background: var(--surface2);
    border: 1px solid var(--border);
    color: var(--text-dim);
    cursor: pointer;
    transition: all 0.15s ease;
    flex-shrink: 0;
  }

  .back-btn:hover {
    background: var(--surface3);
    color: var(--text);
    border-color: var(--text-muted);
  }

  .page-subtitle {
    font-size: 12px;
    margin-top: 1px;
  }

  .search-bar {
    display: flex;
    gap: 8px;
    padding: 12px 28px;
    border-bottom: 1px solid var(--border);
    align-items: center;
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .search-input-wrap {
    position: relative;
    flex: 1;
    min-width: 180px;
  }

  .search-icon {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-muted);
    pointer-events: none;
  }

  .search-input {
    padding-left: 32px;
  }

  .filter-select { width: 140px; }

  .results-header {
    margin-bottom: 12px;
  }

  .results-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    gap: 10px;
  }

  .mod-card {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 14px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    transition: all 0.15s ease;
  }

  .mod-card:hover {
    border-color: var(--text-muted);
    background: var(--surface2);
  }

  .mod-icon-wrap {
    flex-shrink: 0;
  }

  .mod-icon {
    width: 48px;
    height: 48px;
    border-radius: var(--radius-sm);
    object-fit: cover;
    background: var(--surface2);
  }

  .mod-icon-placeholder {
    width: 48px;
    height: 48px;
    border-radius: var(--radius-sm);
    background: var(--surface2);
    border: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
  }

  .mod-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .mod-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mod-desc {
    font-size: 11px;
    line-height: 1.4;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .mod-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
    flex-wrap: wrap;
  }

  .meta-chip {
    display: flex;
    align-items: center;
    gap: 3px;
    font-size: 10px;
    color: var(--text-muted);
  }

  .category-tag {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 100px;
    background: var(--surface2);
    color: var(--text-muted);
    border: 1px solid var(--border);
  }

  .install-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 6px 12px;
    border-radius: var(--radius-sm);
    font-size: 12px;
    font-weight: 600;
    background: rgba(var(--accent-rgb), 0.12);
    color: var(--accent);
    border: 1px solid rgba(var(--accent-rgb), 0.3);
    cursor: pointer;
    transition: all 0.15s ease;
    white-space: nowrap;
    flex-shrink: 0;
    align-self: center;
  }

  .install-btn:hover:not(:disabled) {
    background: rgba(var(--accent-rgb), 0.22);
    border-color: var(--accent);
  }

  .install-btn:disabled,
  .install-btn.installing {
    opacity: 0.7;
    cursor: not-allowed;
  }

  .mini-spinner {
    width: 11px;
    height: 11px;
    border: 2px solid rgba(var(--accent-rgb), 0.3);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin {
    to { transform: rotate(360deg) }
  }

  .load-more {
    display: flex;
    justify-content: center;
    padding: 24px 0;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
    padding: 64px 20px;
    text-align: center;
  }

  /* Skeletons */
  .skeleton-card {
    pointer-events: none;
  }

  .skel {
    background: linear-gradient(90deg, var(--surface2) 25%, var(--surface3) 50%, var(--surface2) 75%);
    background-size: 200% 100%;
    animation: shimmer 1.2s infinite;
    border-radius: var(--radius-sm);
  }

  @keyframes shimmer {
    0%   { background-position: 200% 0 }
    100% { background-position: -200% 0 }
  }

  .skel-icon {
    width: 48px;
    height: 48px;
    border-radius: var(--radius-sm);
    flex-shrink: 0;
  }

  .skel-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding-top: 4px;
  }

  .skel-title {
    height: 13px;
    width: 60%;
  }

  .skel-line {
    height: 11px;
    width: 90%;
  }

  .skel-line.short {
    width: 50%;
  }

  /* Toast */
  .toast {
    position: fixed;
    bottom: 24px;
    right: 24px;
    z-index: 200;
    background: var(--surface2);
    border: 1px solid rgba(52, 211, 153, 0.4);
    color: var(--success);
    padding: 10px 18px;
    border-radius: var(--radius);
    font-size: 13px;
    font-weight: 500;
    box-shadow: 0 4px 20px rgba(0,0,0,0.4);
    animation: slideUp 0.2s ease;
  }

  .toast.toast-error {
    border-color: rgba(248, 113, 113, 0.4);
    color: var(--error);
  }

  @keyframes slideUp {
    from { transform: translateY(8px); opacity: 0 }
    to   { transform: translateY(0);   opacity: 1 }
  }
</style>
