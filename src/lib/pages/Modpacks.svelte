<script>
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { manifest, currentPage, config } from '../../store.js'
  import { loaderColor, loaderIcon } from '../loaderIcons.js'
  import { t } from '../i18n.js'
  import NovaSpinner from '../components/NovaSpinner.svelte'
  import Select from '../components/Select.svelte'

  let mfst = null
  manifest.subscribe(v => (mfst = v))

  let query = ''
  let gameVersion = ''
  let hits = []
  let totalHits = 0
  let offset = 0
  let loading = false
  let searchTimeout = null

  // Detail panel
  let detailHit = null
  let detailProject = null
  let detailVersions = []
  let loadingDetail = false
  let detailTab = 'overview'

  // Install state: { [versionId]: 'installing' | 'done' | 'error' }
  let installState = {}
  // Wizard-style install modal
  let wizardOpen = false
  let wizardStep  = ''    // current step text
  let wizardPct   = 0     // 0..1
  let wizardDone  = false
  let wizardError = null

  onMount(() => search())

  $: releaseVersions = mfst?.versions?.filter(v => v.type === 'release').slice(0, 30) || []

  function onQueryChange() {
    clearTimeout(searchTimeout)
    searchTimeout = setTimeout(() => { offset = 0; search() }, 400)
  }

  async function search() {
    loading = true
    try {
      const result = await invoke('search_modpacks', {
        query,
        gameVersion: gameVersion || null,
        offset,
      })
      hits = result.hits
      totalHits = result.total_hits
    } catch (e) {
      console.error(e)
    } finally {
      loading = false
    }
  }

  async function prevPage() {
    if (offset < 20) return
    offset -= 20
    await search()
  }

  async function nextPage() {
    if (offset + 20 >= totalHits) return
    offset += 20
    await search()
  }

  async function openDetail(hit) {
    detailHit = hit
    detailProject = null
    detailVersions = []
    loadingDetail = true
    detailTab = 'overview'
    try {
      const [proj, vers] = await Promise.all([
        invoke('get_modrinth_project', { projectId: hit.project_id }),
        invoke('get_modrinth_versions', { projectId: hit.project_id, gameVersion: null, loader: null }),
      ])
      detailProject = proj
      detailVersions = vers
    } catch (e) {
      console.error(e)
    } finally {
      loadingDetail = false
    }
  }

  function closeDetail() {
    detailHit = null
    detailProject = null
    detailVersions = []
  }

  function pollProgress(instanceId, onStep) {
    return new Promise((resolve, reject) => {
      const iv = setInterval(async () => {
        try {
          const p = await invoke('get_install_progress', { instanceId })
          if (!p) { clearInterval(iv); resolve(); return }
          if (onStep) onStep(p.step, p.percent)
          if (p.done) {
            clearInterval(iv)
            if (p.error) reject(new Error(p.error))
            else resolve()
          }
        } catch (e) { clearInterval(iv); reject(e) }
      }, 400)
    })
  }

  async function installVersion(ver) {
    if (installState[ver.id]) return
    installState[ver.id] = 'installing'
    installState = installState

    wizardOpen  = true
    wizardDone  = false
    wizardError = null
    wizardStep  = 'Erstelle Instanz…'
    wizardPct   = 0

    try {
      // Phase 1: Instanz erstellen + Mod-Dateien herunterladen
      const instanceId = await invoke('install_modpack', {
        versionId: ver.id,
        name: detailHit?.title || null,
        iconUrl: detailHit?.icon_url || null,
      })

      // Config refreshen damit Instanz sofort in der Liste erscheint
      invoke('get_config').then(cfg => config.set(cfg))

      await pollProgress(instanceId, (step, pct) => {
        wizardStep = step
        wizardPct  = pct
      })

      // Phase 2: Minecraft + Loader (wie im Wizard)
      wizardStep = 'Minecraft wird installiert…'
      wizardPct  = 0
      await invoke('prepare_instance', { instanceId })
      await pollProgress(instanceId, (step, pct) => {
        wizardStep = step
        wizardPct  = pct
      })

      // Fertig
      const finalCfg = await invoke('get_config')
      config.set(finalCfg)

      installState[ver.id] = 'done'
      installState = installState
      wizardDone = true

      setTimeout(() => {
        wizardOpen = false
        currentPage.set('instances')
      }, 1800)

    } catch (e) {
      installState[ver.id] = 'error'
      installState = installState
      wizardError = String(e)
    }
  }

  function formatNum(n) {
    if (n >= 1000000) return `${(n / 1000000).toFixed(1)}M`
    if (n >= 1000) return `${(n / 1000).toFixed(0)}K`
    return String(n)
  }

  function versionTypeColor(t) {
    return t === 'release' ? 'var(--success)' : t === 'beta' ? '#f59e0b' : 'var(--text-muted)'
  }

  // Simple markdown renderer (headings, paragraphs, bold, italic, lists)
  function renderMarkdown(text) {
    if (!text) return ''
    const lines = text.split('\n')
    const out = []
    let inList = false
    for (const raw of lines) {
      const t = raw.trim()
      if (!t) { if (inList) { out.push('</ul>'); inList = false } continue }
      if (t.startsWith('# '))  { out.push(`<h2 class="md-h2">${esc(t.slice(2))}</h2>`); continue }
      if (t.startsWith('## ')) { out.push(`<h3 class="md-h3">${esc(t.slice(3))}</h3>`); continue }
      if (t.startsWith('### ')){ out.push(`<h4 class="md-h4">${esc(t.slice(4))}</h4>`); continue }
      if (/^[*\-] /.test(t)) {
        if (!inList) { out.push('<ul class="md-list">'); inList = true }
        out.push(`<li>${inline(t.slice(2))}</li>`)
        continue
      }
      if (inList) { out.push('</ul>'); inList = false }
      out.push(`<p class="md-p">${inline(t)}</p>`)
    }
    if (inList) out.push('</ul>')
    return out.join('\n')
  }

  function esc(s) { return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;') }
  function inline(s) {
    return esc(s)
      .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
      .replace(/\*(.+?)\*/g, '<em>$1</em>')
      .replace(/`([^`]+)`/g, '<code class="md-code">$1</code>')
  }
</script>

<div class="page">
  <div class="page-header">
    <h1 class="page-title">Modpacks</h1>
    <div class="header-right">
      <span class="text-muted" style="font-size:12px">{totalHits.toLocaleString('de-DE')} Ergebnisse</span>
    </div>
  </div>

  <!-- Search bar -->
  <div class="search-bar">
    <div class="search-input-wrap">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="15" height="15" class="search-icon"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
      <input
        class="search-input"
        type="text"
        placeholder="Modpacks suchen..."
        bind:value={query}
        on:input={onQueryChange}
      />
    </div>
    <div class="filter-select">
      <Select
        bind:value={gameVersion}
        options={[{ value: '', label: 'Alle Versionen' }, ...releaseVersions.map(v => ({ value: v.id, label: v.id }))]}
        on:change={() => { offset = 0; search() }}
      />
    </div>
  </div>

  <div class="page-body">
    {#if loading}
      <div class="section-spinner">
        <NovaSpinner size={56} label="Sucht..." />
      </div>
    {:else if hits.length === 0}
      <div class="empty text-muted">Keine Modpacks gefunden</div>
    {:else}
      <div class="grid">
        {#each hits as pack}
          <!-- svelte-ignore a11y-click-events-have-key-events -->
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div class="pack-card card card-hover" on:click={() => openDetail(pack)}>
            <!-- Banner -->
            <div class="pack-banner">
              {#if pack.featured_gallery}
                <img src={pack.featured_gallery} alt={pack.title} class="banner-img" />
              {:else}
                <div class="banner-fallback" style="background: {pack.color ? `#${Math.abs(pack.color).toString(16).padStart(6,'0')}22` : 'var(--surface2)'}">
                  {#if pack.icon_url}
                    <img src={pack.icon_url} alt="" class="banner-fallback-icon" />
                  {:else}
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="48" height="48" style="color:var(--border)"><rect x="1" y="3" width="15" height="13"/><polygon points="16 8 20 8 23 11 23 16 16 16 16 8"/><circle cx="5.5" cy="18.5" r="2.5"/><circle cx="18.5" cy="18.5" r="2.5"/></svg>
                  {/if}
                </div>
              {/if}
            </div>
            {#if pack.icon_url && pack.featured_gallery}
              <img src={pack.icon_url} alt="" class="pack-icon-overlay" />
            {/if}

            <!-- Info -->
            <div class="pack-body" class:has-icon={!!pack.icon_url}>
              <div class="pack-title">{pack.title}</div>
              <div class="pack-desc text-muted">{pack.description}</div>

              <div class="pack-tags">
                {#each pack.categories.slice(0, 3) as cat}
                  <span class="badge badge-muted">{cat}</span>
                {/each}
                {#if pack.versions && pack.versions.length > 0}
                  <span class="badge badge-info">{pack.versions[pack.versions.length - 1]}</span>
                {/if}
              </div>

              <div class="pack-footer">
                <span class="dl-count text-muted">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                  {formatNum(pack.downloads)}
                </span>
                <span class="text-muted" style="font-size:11px">Klicken zum Öffnen →</span>
              </div>
            </div>
          </div>
        {/each}
      </div>

      <!-- Pagination -->
      <div class="pagination">
        <button class="btn btn-ghost btn-sm" on:click={prevPage} disabled={offset === 0}>{$t('common.back')}</button>
        <span class="page-info text-muted">
          {$t('common.pagination', { start: offset + 1, end: Math.min(offset + 20, totalHits), total: totalHits.toLocaleString() })}
        </span>
        <button class="btn btn-ghost btn-sm" on:click={nextPage} disabled={offset + 20 >= totalHits}>{$t('common.next')}</button>
      </div>
    {/if}
  </div>
</div>

<!-- ═══ Detail Overlay ═══ -->
{#if detailHit}
<!-- svelte-ignore a11y-click-events-have-key-events -->
<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="detail-overlay" on:click|self={closeDetail}>
  <div class="detail-panel">
    <!-- Header -->
    <div class="detail-header">
      <button class="back-btn" on:click={closeDetail}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="15" height="15"><polyline points="15 18 9 12 15 6"/></svg>
      </button>
      {#if detailHit.icon_url}
        <img src={detailHit.icon_url} alt="" class="detail-icon" />
      {:else}
        <div class="detail-icon-placeholder">{detailHit.title[0]}</div>
      {/if}
      <div class="detail-title-block">
        <span class="detail-title">{detailHit.title}</span>
        {#if detailHit.author}<span class="detail-author text-muted">{detailHit.author}</span>{/if}
      </div>
      <div class="detail-stat-row">
        <span class="detail-stat-chip">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
          {formatNum(detailHit.downloads)}
        </span>
        <span class="detail-stat-chip">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11"><path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/></svg>
          {formatNum(detailHit.follows || 0)}
        </span>
      </div>
      <div class="detail-header-actions">
        {#if loadingDetail}<div class="detail-spinner"></div>{/if}
      </div>
    </div>

    <!-- Tabs -->
    <div class="detail-tabs">
      <button class="detail-tab" class:active={detailTab === 'overview'} on:click={() => detailTab = 'overview'}>Übersicht</button>
      <button class="detail-tab" class:active={detailTab === 'versions'} on:click={() => detailTab = 'versions'}>
        Versionen
        {#if detailVersions.length > 0}<span class="tab-badge">{detailVersions.length}</span>{/if}
      </button>
    </div>

    <!-- Tab content -->
    <div class="detail-body">

      {#if detailTab === 'overview'}
        <div class="tab-pane">
          <p class="detail-short-desc">{detailHit.description}</p>

          {#if detailProject?.body}
            <div class="md-body">{@html renderMarkdown(detailProject.body)}</div>
          {:else if loadingDetail}
            <div class="text-muted" style="font-size:13px;padding:16px 0">Beschreibung wird geladen…</div>
          {/if}

          {#if detailHit.categories?.length > 0}
            <div class="overview-section">
              <span class="section-label">Kategorien</span>
              <div class="overview-tags">
                {#each detailHit.categories as cat}
                  <span class="badge badge-muted">{cat}</span>
                {/each}
              </div>
            </div>
          {/if}

          {#if detailProject}
            <div class="overview-section">
              <span class="section-label">Links</span>
              <div class="detail-links">
                <a href="https://modrinth.com/modpack/{detailProject.slug}"
                   on:click|preventDefault={() => invoke('open_url', { url: `https://modrinth.com/modpack/${detailProject.slug}` })}
                   class="detail-link">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
                  Modrinth
                </a>
                {#if detailProject.source_url}
                  <a href={detailProject.source_url} on:click|preventDefault={() => invoke('open_url', { url: detailProject.source_url })} class="detail-link">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>
                    Quellcode
                  </a>
                {/if}
              </div>
            </div>
          {/if}
        </div>

      {:else if detailTab === 'versions'}
        <div class="tab-pane">
          {#if loadingDetail}
            <div class="text-muted" style="font-size:13px;padding:16px 0">Versionen werden geladen…</div>
          {:else if detailVersions.length === 0}
            <div class="text-muted" style="font-size:13px;padding:16px 0">Keine Versionen gefunden.</div>
          {:else}
            <div class="versions-list">
              {#each detailVersions as ver}
                {@const state = installState[ver.id]}
                <div class="version-row">
                  <div class="version-left">
                    <div class="version-top-row">
                      <span class="version-number">{ver.version_number}</span>
                      <span class="version-type-badge" style="color:{versionTypeColor(ver.version_type)};border-color:{versionTypeColor(ver.version_type)}22;background:{versionTypeColor(ver.version_type)}11">
                        {ver.version_type}
                      </span>
                    </div>
                    {#if ver.name && ver.name !== ver.version_number}
                      <span class="version-name text-muted">{ver.name}</span>
                    {/if}
                    <div class="version-tags-row">
                      {#each (ver.game_versions || []).slice(0, 5) as gv}
                        <span class="ver-tag">{gv}</span>
                      {/each}
                      {#if (ver.game_versions || []).length > 5}
                        <span class="ver-tag text-muted">+{ver.game_versions.length - 5}</span>
                      {/if}
                      {#each (ver.loaders || []) as l}
                        {@const lc = loaderColor(l)}
                        <span class="ver-tag loader-tag" style="color:{lc};border-color:{lc}33;background:{lc}11">
                          {#if loaderIcon(l)}<span class="loader-icon-sm">{@html loaderIcon(l)}</span>{/if}
                          {l}
                        </span>
                      {/each}
                    </div>
                  </div>
                  <div class="version-right">
                    {#if state === 'done'}
                      <span class="install-done-badge">✓ Installiert</span>
                    {:else if state === 'installing'}
                      <button class="btn btn-primary btn-sm" disabled>
                        <div class="btn-spinner"></div>
                      </button>
                    {:else}
                      <button
                        class="btn btn-primary btn-sm"
                        disabled={wizardOpen}
                        on:click={() => installVersion(ver)}
                      >
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                        Installieren
                      </button>
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {/if}

    </div>
  </div>
</div>
{/if}

<!-- ═══ Install Wizard Modal (exakt wie CreateInstanceWizard Step 3) ═══ -->
{#if wizardOpen}
<div class="wiz-overlay">
  <div class="wiz-modal">
    <div class="wiz-header">
      <div class="wiz-title">Modpack wird installiert</div>
    </div>
    <div class="wiz-body">
      {#if wizardDone}
        <div class="wiz-done">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="44" height="44" style="color:var(--success)"><circle cx="12" cy="12" r="10"/><polyline points="8 12 11 15 16 9"/></svg>
          <p class="wiz-done-text">Instanz bereit!</p>
        </div>
      {:else if wizardError}
        <div class="wiz-error">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="40" height="40" style="color:var(--error)"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          <p class="wiz-error-text">{wizardError}</p>
          <button class="btn btn-ghost" on:click={() => wizardOpen = false}>Schließen</button>
        </div>
      {:else}
        <div class="wiz-progress">
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
          <div class="wiz-step">{wizardStep || '…'}</div>
          <div class="wiz-bar-wrap">
            <div class="wiz-bar-fill" style="width:{Math.round(wizardPct * 100)}%"></div>
          </div>
          <div class="wiz-pct">{Math.round(wizardPct * 100)}%</div>
        </div>
      {/if}
    </div>
  </div>
</div>
{/if}

<style>
  .search-bar {
    display: flex;
    gap: 10px;
    padding: 12px 24px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .search-input-wrap {
    position: relative;
    flex: 1;
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
    width: 100%;
    background: var(--surface2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    color: var(--text);
    padding: 8px 12px 8px 34px;
    font-size: 13px;
    font-family: inherit;
    outline: none;
    transition: border-color var(--transition);
  }
  .search-input:focus { border-color: var(--accent); }

  .filter-select { min-width: 140px; }
  .header-right { display: flex; align-items: center; gap: 12px; }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 16px;
    margin-bottom: 24px;
  }

  .section-spinner { display: flex; justify-content: center; padding: 64px 0; }

  .pack-card { display: flex; flex-direction: column; overflow: visible; position: relative; }
  .card-hover { cursor: pointer; transition: all var(--transition); }
  .card-hover:hover { border-color: rgba(var(--accent-rgb),0.4); transform: translateY(-1px); box-shadow: 0 4px 16px rgba(0,0,0,0.25); }

  .pack-banner { height: 130px; flex-shrink: 0; overflow: hidden; background: var(--surface2); border-radius: var(--radius) var(--radius) 0 0; position: relative; display: flex; align-items: center; justify-content: center; }
  .banner-img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .banner-fallback { width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; }
  .banner-fallback-icon { width: 56px; height: 56px; object-fit: contain; border-radius: 8px; opacity: 0.7; }
  .pack-icon-overlay { position: absolute; top: calc(130px - 18px); left: 14px; width: 36px; height: 36px; border-radius: 8px; border: 2px solid var(--surface); object-fit: contain; image-rendering: pixelated; z-index: 2; background: var(--surface2); }

  .pack-body { padding: 14px; display: flex; flex-direction: column; gap: 8px; flex: 1; }
  .pack-body.has-icon { padding-top: 26px; }
  .pack-title { font-size: 14px; font-weight: 600; color: var(--text); }
  .pack-desc { font-size: 12px; line-height: 1.4; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .pack-tags { display: flex; flex-wrap: wrap; gap: 4px; }
  .pack-footer { display: flex; align-items: center; justify-content: space-between; margin-top: auto; }
  .dl-count { display: flex; align-items: center; gap: 4px; font-size: 12px; }

  .empty { padding: 64px; text-align: center; font-size: 14px; }
  .pagination { display: flex; align-items: center; justify-content: center; gap: 16px; padding: 16px 0; }
  .page-info { font-size: 13px; }

  /* ─── Detail Overlay ─── */
  .detail-overlay {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.7);
    z-index: 200;
    display: flex; align-items: center; justify-content: center;
    padding: 16px;
  }

  .detail-panel {
    width: 100%; max-width: 800px; max-height: 90vh;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    display: flex; flex-direction: column;
    overflow: hidden;
  }

  /* Header */
  .detail-header {
    display: flex; align-items: center; gap: 10px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .back-btn {
    width: 28px; height: 28px;
    border-radius: 6px;
    background: var(--surface2); border: 1px solid var(--border);
    display: flex; align-items: center; justify-content: center;
    cursor: pointer; color: var(--text); flex-shrink: 0;
    transition: background var(--transition);
  }
  .back-btn:hover { background: var(--surface3); }

  .detail-icon {
    width: 36px; height: 36px; border-radius: 8px;
    object-fit: contain; image-rendering: pixelated;
    flex-shrink: 0;
  }
  .detail-icon-placeholder {
    width: 36px; height: 36px; border-radius: 8px;
    background: var(--surface2); border: 1px solid var(--border);
    display: flex; align-items: center; justify-content: center;
    font-size: 16px; font-weight: 700; color: var(--text-muted);
    flex-shrink: 0;
  }

  .detail-title-block { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .detail-title { font-size: 13px; font-weight: 600; color: var(--text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .detail-author { font-size: 11px; }

  .detail-stat-row { display: flex; align-items: center; gap: 6px; margin-left: auto; flex-shrink: 0; }
  .detail-stat-chip {
    display: flex; align-items: center; gap: 4px;
    font-size: 11px; color: var(--text-muted);
    background: var(--surface2); border: 1px solid var(--border);
    border-radius: 100px; padding: 2px 8px;
  }

  .detail-header-actions { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }
  .detail-spinner {
    width: 16px; height: 16px; border-radius: 50%;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
  }

  /* Tabs */
  .detail-tabs {
    display: flex; gap: 0;
    border-bottom: 1px solid var(--border);
    padding: 0 14px;
    flex-shrink: 0;
  }
  .detail-tab {
    padding: 10px 14px;
    font-size: 13px; font-weight: 500;
    color: var(--text-muted);
    background: none; border: none; cursor: pointer;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    transition: color var(--transition), border-color var(--transition);
    display: flex; align-items: center; gap: 6px;
  }
  .detail-tab:hover { color: var(--text); }
  .detail-tab.active { color: var(--accent); border-bottom-color: var(--accent); }
  .tab-badge {
    background: var(--surface2); border: 1px solid var(--border);
    color: var(--text-muted); font-size: 10px;
    padding: 1px 5px; border-radius: 100px;
  }

  /* Body */
  .detail-body { flex: 1; overflow-y: auto; }
  .tab-pane { padding: 16px 20px; display: flex; flex-direction: column; gap: 14px; }

  .detail-short-desc { font-size: 13px; color: var(--text-muted); line-height: 1.5; }

  .overview-section { display: flex; flex-direction: column; gap: 8px; }
  .section-label { font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.08em; color: var(--text-muted); }
  .overview-tags { display: flex; flex-wrap: wrap; gap: 4px; }

  .detail-links { display: flex; flex-wrap: wrap; gap: 6px; }
  .detail-link {
    display: inline-flex; align-items: center; gap: 5px;
    font-size: 12px; color: var(--accent); text-decoration: none;
    background: rgba(var(--accent-rgb),0.08); border: 1px solid rgba(var(--accent-rgb),0.2);
    border-radius: 6px; padding: 4px 10px;
    transition: background var(--transition);
  }
  .detail-link:hover { background: rgba(var(--accent-rgb),0.15); }

  /* Markdown */
  .md-body { font-size: 13px; line-height: 1.6; color: var(--text); }
  .md-body :global(.md-h2) { font-size: 15px; font-weight: 700; margin: 14px 0 6px; color: var(--text); }
  .md-body :global(.md-h3) { font-size: 13px; font-weight: 600; margin: 10px 0 4px; color: var(--text); }
  .md-body :global(.md-h4) { font-size: 12px; font-weight: 600; margin: 8px 0 4px; color: var(--text-muted); }
  .md-body :global(.md-p)  { margin: 4px 0; color: var(--text-muted); }
  .md-body :global(.md-list) { margin: 4px 0 4px 16px; }
  .md-body :global(.md-code) { background: var(--surface2); border-radius: 3px; padding: 1px 4px; font-family: monospace; font-size: 12px; }

  /* ─── Wizard Install Modal ─── */
  .wiz-overlay {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.65);
    display: flex; align-items: center; justify-content: center;
    z-index: 300;
    animation: fadeIn 0.15s ease;
  }
  .wiz-modal {
    background: var(--surface2);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    width: 100%; max-width: 520px;
    margin: 16px;
    display: flex; flex-direction: column;
    animation: slideIn 0.18s ease;
    overflow: hidden;
  }
  .wiz-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 22px 28px 18px;
    border-bottom: 1px solid var(--border);
  }
  .wiz-title { font-size: 16px; font-weight: 700; color: var(--text); }
  .wiz-body {
    padding: 32px 28px;
    display: flex; flex-direction: column;
    align-items: center; justify-content: center;
    gap: 16px; min-height: 280px;
  }
  .wiz-progress { width: 100%; display: flex; flex-direction: column; gap: 10px; align-items: center; }
  .wiz-logo-wrap {
    width: 96px; height: 96px;
    display: flex; align-items: center; justify-content: center;
    margin-bottom: 8px;
    animation: wizFloat 3s ease-in-out 1.2s infinite;
  }
  .wiz-logo-wrap svg { width: 96px; height: 96px; }
  .wiz-step { font-size: 13px; color: var(--text-dim); text-align: center; min-height: 20px; }
  .wiz-bar-wrap {
    width: 100%; max-width: 320px; height: 6px;
    background: var(--surface3); border-radius: 100px; overflow: hidden;
  }
  .wiz-bar-fill { height: 100%; background: var(--accent); border-radius: 100px; transition: width 0.3s ease; }
  .wiz-pct { font-size: 11px; color: var(--text-muted); font-variant-numeric: tabular-nums; }
  .wiz-done { display: flex; flex-direction: column; align-items: center; gap: 12px; animation: fadeIn 0.25s ease; }
  .wiz-done-text { font-size: 14px; font-weight: 600; color: var(--text); }
  .wiz-error { display: flex; flex-direction: column; align-items: center; gap: 12px; }
  .wiz-error-text { font-size: 13px; color: var(--error); text-align: center; max-width: 360px; }

  .wr { transform-box: fill-box; transform-origin: center; animation: wizBuild 0.45s cubic-bezier(0.34,1.56,0.64,1) both; }
  .wr7 { animation-delay: 0.00s; } .wr6 { animation-delay: 0.10s; }
  .wr5 { animation-delay: 0.20s; } .wr4 { animation-delay: 0.30s; }
  .wr3 { animation-delay: 0.40s; } .wr2 { animation-delay: 0.50s; }
  .wr1 { animation-delay: 0.60s; }

  @keyframes wizBuild {
    0%   { opacity: 0; transform: scale(0.2); }
    70%  { opacity: 1; transform: scale(1.08); }
    100% { opacity: 1; transform: scale(1); }
  }
  @keyframes wizFloat {
    0%, 100% { transform: translateY(0); }
    50%       { transform: translateY(-5px); }
  }
  @keyframes fadeIn  { from { opacity: 0 } to { opacity: 1 } }
  @keyframes slideIn { from { transform: translateY(-12px); opacity: 0 } to { transform: translateY(0); opacity: 1 } }

  /* Versions list */
  .versions-list { display: flex; flex-direction: column; gap: 6px; }
  .version-row {
    display: flex; align-items: center; justify-content: space-between; gap: 12px;
    padding: 10px 12px;
    background: var(--surface2); border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }
  .version-left { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .version-top-row { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  .version-number { font-size: 13px; font-weight: 600; color: var(--text); }
  .version-name { font-size: 11px; }
  .version-type-badge {
    font-size: 10px; padding: 1px 6px; border-radius: 100px;
    border: 1px solid;
  }
  .version-tags-row { display: flex; flex-wrap: wrap; gap: 3px; }
  .ver-tag {
    font-size: 10px; padding: 1px 5px; border-radius: 4px;
    background: var(--surface3); border: 1px solid var(--border);
    color: var(--text-muted);
  }
  .loader-tag { display: inline-flex; align-items: center; gap: 3px; border: 1px solid; }
  .loader-icon-sm { display: flex; align-items: center; width: 10px; height: 10px; }
  .loader-icon-sm :global(svg) { width: 10px; height: 10px; }

  .version-right { flex-shrink: 0; }

  .install-done-badge {
    font-size: 11px; color: var(--success); font-weight: 600;
    display: flex; align-items: center; gap: 4px;
  }

  .btn-spinner {
    width: 11px; height: 11px;
    border: 1.5px solid rgba(255,255,255,0.4);
    border-top-color: #fff;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    display: inline-block;
  }

  @keyframes spin { to { transform: rotate(360deg) } }
</style>
