<script>
  import { invoke } from '@tauri-apps/api/core'
  import { config, manifest } from '../../store.js'
  import Checkbox from '../components/Checkbox.svelte'
  import { t } from '../../lib/i18n.js'

  let cfg = null
  let man = null
  let loading = false
  let searchQuery = ''

  config.subscribe(v => (cfg = v))
  manifest.subscribe(v => (man = v))

  $: selectedVersion = cfg?.instances?.find(i => i.id === cfg?.active_instance_id)?.version || ''

  $: filteredVersions = (() => {
    if (!man?.versions) return []
    let versions = man.versions.filter(v => {
      switch (v.version_type) {
        case 'release': return true
        case 'snapshot': return cfg?.show_snapshots
        case 'old_alpha': return cfg?.show_old_alpha
        case 'old_beta': return cfg?.show_old_beta
        default: return true
      }
    })
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase()
      versions = versions.filter(v => v.id.toLowerCase().includes(q))
    }
    return versions
  })()

  async function selectVersion(id) {
    try {
      const updated = await invoke('get_config')
      // Find active instance and set version
      if (updated.active_instance_id) {
        const inst = updated.instances.find(i => i.id === updated.active_instance_id)
        if (inst) inst.version = id
        await invoke('save_config', { config: updated })
        config.set(updated)
      }
    } catch (e) {
      console.error(e)
    }
  }

  async function refreshManifest() {
    loading = true
    try {
      const m = await invoke('refresh_manifest')
      manifest.set(m)
    } catch (e) {
      console.error(e)
    } finally {
      loading = false
    }
  }

  async function toggleFilter(key) {
    try {
      const updated = await invoke('get_config')
      updated[key] = !updated[key]
      await invoke('save_config', { config: updated })
      config.set(updated)
    } catch (e) {
      console.error(e)
    }
  }

  function getBadgeClass(type) {
    switch (type) {
      case 'release': return 'badge-success'
      case 'snapshot': return 'badge-warning'
      default: return 'badge-muted'
    }
  }

  function getTypeLabel(type) {
    switch (type) {
      case 'release': return 'Release'
      default: return type
    }
  }
</script>

<div class="page">
  <div class="page-header">
    <h1 class="page-title">{$t('versions.title')}</h1>
    <button class="btn btn-ghost" on:click={refreshManifest} disabled={loading}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"
        class:spinning={loading}>
        <polyline points="1 4 1 10 7 10"/><polyline points="23 20 23 14 17 14"/>
        <path d="M20.49 9A9 9 0 0 0 5.64 5.64L1 10m22 4l-4.64 4.36A9 9 0 0 1 3.51 15"/>
      </svg>
      {loading ? $t('versions.loading') : $t('versions.refresh')}
    </button>
  </div>

  <!-- Filters -->
  <div class="filters-bar">
    <input
      class="input search-input"
      type="text"
      placeholder={$t('versions.searchPlaceholder')}
      bind:value={searchQuery}
    />
    <div class="filter-toggles">
      <Checkbox checked={cfg?.show_snapshots || false} on:change={() => toggleFilter('show_snapshots')}>{$t('versions.snapshots')}</Checkbox>
      <Checkbox checked={cfg?.show_old_alpha || false} on:change={() => toggleFilter('show_old_alpha')}>{$t('versions.alpha')}</Checkbox>
      <Checkbox checked={cfg?.show_old_beta || false} on:change={() => toggleFilter('show_old_beta')}>{$t('versions.beta')}</Checkbox>
    </div>
  </div>

  <div class="versions-list-wrapper">
    {#if !man}
      <div class="loading-state">
        <div class="spinner"></div>
        <p class="text-dim">{$t('versions.versionsLoading')}</p>
      </div>
    {:else if filteredVersions.length === 0}
      <div class="loading-state">
        <p class="text-dim">{$t('versions.noVersions')}</p>
      </div>
    {:else}
      <div class="versions-list">
        {#each filteredVersions.slice(0, 500) as v (v.id)}
          {@const isSelected = selectedVersion === v.id}
          <button
            class="version-item"
            class:selected={isSelected}
            on:click={() => selectVersion(v.id)}
          >
            <span class="version-id" class:selected-text={isSelected}>{v.id}</span>
            <div class="version-meta">
              <span class="badge {getBadgeClass(v.version_type)}">
                {v.version_type === 'snapshot' ? $t('versions.snapshots') : v.version_type === 'old_alpha' ? $t('versions.alpha') : v.version_type === 'old_beta' ? $t('versions.beta') : getTypeLabel(v.version_type)}
              </span>
              <span class="version-date text-muted">{v.release_time?.slice(0, 10) || ''}</span>
            </div>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .page {
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .filters-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 28px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .search-input {
    max-width: 220px;
    padding: 7px 12px;
    font-size: 13px;
  }

  .filter-toggles {
    display: flex;
    gap: 16px;
    align-items: center;
  }

  .toggle-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--text-dim);
    cursor: pointer;
    transition: color var(--transition);
  }

  .toggle-label:hover {
    color: var(--text);
  }


  .versions-list-wrapper {
    flex: 1;
    overflow-y: auto;
    padding: 8px 20px;
  }

  .versions-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .version-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all var(--transition);
    background: none;
    border: 1px solid transparent;
    color: var(--text);
    width: 100%;
    text-align: left;
  }

  .version-item:hover {
    background: var(--surface2);
    border-color: var(--border);
  }

  .version-item.selected {
    background: rgba(var(--accent-rgb), 0.12);
    border-color: rgba(var(--accent-rgb), 0.35);
  }

  .version-id {
    font-size: 13px;
    font-weight: 500;
    color: var(--text);
  }

  .version-id.selected-text {
    color: var(--accent);
  }

  .version-meta {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .version-date {
    font-size: 11px;
    min-width: 85px;
    text-align: right;
  }

  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 60px;
    color: var(--text-muted);
  }

  .spinner {
    width: 28px;
    height: 28px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  :global(.spinning) {
    animation: spin 0.8s linear infinite;
  }
</style>
