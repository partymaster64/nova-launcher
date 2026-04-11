<script>
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { config, accounts, currentPage, detailInstanceId, crashEvent } from '../../store.js'
  import InstanceCard from '../components/InstanceCard.svelte'
  import LaunchOverlay from '../components/LaunchOverlay.svelte'
  import NovaSpinner from '../components/NovaSpinner.svelte'
  import { t } from '../../lib/i18n.js'

  let cfg = null
  let accs = []

  config.subscribe(v => (cfg = v))
  accounts.subscribe(v => (accs = v))

  let runningIds  = []
  let launchingId = null

  // Clear LaunchOverlay if crash modal appears
  $: if ($crashEvent) launchingId = null

  onMount(() => {
    const tick = async () => {
      try {
        const r = await invoke('get_running_instances')
        runningIds = r
        if (launchingId && r.includes(launchingId)) launchingId = null
      } catch (_) {}
    }
    tick()
    const iv = setInterval(tick, 750)
    return () => clearInterval(iv)
  })

  let featuredModpacks = []
  let loadingModpacks = true
  let jumpBackWorlds = []  // WorldInfo + instanceId/Name
  let jumpBackServers = [] // ServerNbt + instanceId/Name
  let jumpServerPings = {} // ip -> { loading, result, error }

  const HIDDEN_KEY = 'nova-jumpback-hidden'
  let hiddenEntries = new Set(JSON.parse(localStorage.getItem(HIDDEN_KEY) || '[]'))
  let serverHistory = {} // instanceId:serverHost -> epoch_ms, loaded from backend

  function dismissEntry(key) {
    hiddenEntries.add(key)
    localStorage.setItem(HIDDEN_KEY, JSON.stringify([...hiddenEntries]))
    jumpBackWorlds = jumpBackWorlds.filter(w => !hiddenEntries.has(`world:${w.instanceId}:${w.name}`))
    jumpBackServers = jumpBackServers.filter(s => !hiddenEntries.has(`server:${s.instanceId}:${s.ip}`))
  }

  $: activeAccount = accs.find(a => a.uuid === cfg?.active_account_uuid)
  $: recentInstances = cfg?.instances
    ? [...cfg.instances].sort((a, b) => (b.last_played || '').localeCompare(a.last_played || '')).slice(0, 6)
    : []

  onMount(async () => {
    try {
      const result = await invoke('search_modpacks', { query: '', gameVersion: null, offset: 0 })
      featuredModpacks = result.hits.slice(0, 8)
    } catch (e) {
      console.error('Modpacks:', e)
    } finally {
      loadingModpacks = false
    }

    // Load server play history from backend (recorded when "Connecting to" appears in game log)
    try { serverHistory = await invoke('get_server_play_history') } catch (_) {}

    // Load jump-back-in from all instances, filter to last 5 days, sort most recent first
    if (cfg?.instances?.length) {
      const FIVE_DAYS = 5 * 24 * 60 * 60 * 1000
      const now = Date.now()
      const allWorlds = []
      const allServers = []

      await Promise.all(cfg.instances.map(async inst => {
        try {
          const details = await invoke('get_instance_details', { instanceId: inst.id })
          for (const w of (details.worlds || [])) {
            if (w.last_played_ms && (now - w.last_played_ms) <= FIVE_DAYS)
              allWorlds.push({ ...w, instanceId: inst.id, instanceName: inst.name })
          }
          // servers.dat has no per-server timestamp — use Nova's log-based history
          for (const s of (details.servers || [])) {
            // Normalize ip: strip default port so "host:25565" matches log key "host"
            const host = s.ip.replace(/:25565$/, '')
            const historyKey = `${inst.id}:${host}`
            const lastPlayed = serverHistory[historyKey] || 0
            if (lastPlayed && (now - lastPlayed) <= FIVE_DAYS)
              allServers.push({ ...s, instanceId: inst.id, instanceName: inst.name, last_played_ms: lastPlayed })
          }
        } catch (_) {}
      }))

      allWorlds.sort((a, b) => (b.last_played_ms || 0) - (a.last_played_ms || 0))
      allServers.sort((a, b) => (b.last_played_ms || 0) - (a.last_played_ms || 0))

      jumpBackWorlds = allWorlds.filter(w => !hiddenEntries.has(`world:${w.instanceId}:${w.name}`))
      jumpBackServers = allServers.filter(s => !hiddenEntries.has(`server:${s.instanceId}:${s.ip}`))

      // Ping servers async for MOTD
      for (const s of jumpBackServers) {
        jumpServerPings[s.ip] = { loading: true, result: null }
        jumpServerPings = jumpServerPings
        invoke('ping_server', { address: s.ip })
          .then(r => { jumpServerPings[s.ip] = { loading: false, result: r }; jumpServerPings = jumpServerPings })
          .catch(() => { jumpServerPings[s.ip] = { loading: false, result: null }; jumpServerPings = jumpServerPings })
      }
    }
  })

  async function launchInstance(id) {
    if (!activeAccount) return
    launchingId = id
    try {
      await invoke('launch_instance', { instanceId: id })
    } catch (e) {
      console.error('Launch error:', e)
      launchingId = null
    }
  }

  async function launchJumpBack(instanceId, targetType, target) {
    if (!activeAccount) return
    launchingId = instanceId
    try {
      await invoke('launch_with_quickplay', { instanceId, targetType, target })
    } catch (e) {
      console.error('Jump back launch error:', e)
      launchingId = null
    }
  }

  async function stopInstance(id) {
    try { await invoke('kill_instance', { instanceId: id }) } catch (e) { console.error(e) }
  }

  function openDetail(id) {
    detailInstanceId.set(id)
    currentPage.set('instance-detail')
  }

  function formatDownloads(n) {
    if (n >= 1000000) return `${(n / 1000000).toFixed(1)}M`
    if (n >= 1000) return `${(n / 1000).toFixed(0)}K`
    return String(n)
  }

  function formatLastPlayed(ms) {
    if (!ms) return null
    const d = new Date(ms)
    const today = new Date()
    const diff = Math.floor((today - d) / 86400000)
    if (diff === 0) return $t('common.today')
    if (diff === 1) return $t('common.yesterday')
    if (diff < 7) return $t('common.daysAgo', { days: diff })
    return d.toLocaleDateString(undefined, { day: '2-digit', month: '2-digit', year: 'numeric' })
  }
</script>

<div class="page">
  <!-- Header -->
  <div class="page-header">
    <div>
      <h1 class="page-title">
        {#if activeAccount}
          {@html $t('home.welcome', { name: `<span class="text-accent">${activeAccount.username}</span>` })}
        {:else}
          {$t('home.title')}
        {/if}
      </h1>
      <p class="header-sub text-muted">
        {(cfg?.instances?.length || 0) === 1
          ? $t('home.instanceCount', { count: 1 })
          : $t('home.instanceCountPlural', { count: cfg?.instances?.length || 0 })}
      </p>
    </div>
  </div>

  <div class="page-body">

    <!-- Jump Back In -->
    {#if jumpBackWorlds.length > 0 || jumpBackServers.length > 0}
      <section class="section">
        <div class="section-header">
          <span class="section-label">{$t('home.recentlyPlayed')}</span>
        </div>
        <div class="jb-grid">
          {#each jumpBackWorlds as w}
            <div class="jb-row card">
              <div class="jb-icon">
                {#if w.icon}
                  <img src="data:image/png;base64,{w.icon}" alt="" class="jb-icon-img" />
                {:else}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="32" height="32" style="color:var(--accent)"><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>
                {/if}
              </div>
              <div class="jb-info">
                <span class="jb-label">{w.display_name || w.name}</span>
                <span class="jb-sub text-muted">{w.instanceName}{w.last_played_ms ? ' · ' + formatLastPlayed(w.last_played_ms) : ''}</span>
              </div>
              <button class="btn btn-ghost btn-sm" on:click={() => launchJumpBack(w.instanceId, 'world', w.name)} disabled={!activeAccount || !!launchingId}>
                <svg viewBox="0 0 24 24" fill="currentColor" width="11" height="11"><polygon points="5 3 19 12 5 21 5 3"/></svg>
                {$t('common.play')}
              </button>
              <button class="jb-dismiss" on:click={() => dismissEntry(`world:${w.instanceId}:${w.name}`)} title={$t('common.remove')}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="12" height="12"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
              </button>
            </div>
          {/each}
          {#each jumpBackServers as s}
            {@const ping = jumpServerPings[s.ip]}
            {@const icon = ping?.result?.icon || s.icon}
            <div class="jb-server-row card">
              <div class="jb-icon">
                {#if icon}
                  <img src="data:image/png;base64,{icon}" alt="" class="jb-icon-img" />
                {:else}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="32" height="32" style="color:var(--accent)"><rect x="2" y="3" width="20" height="4" rx="1"/><rect x="2" y="10" width="20" height="4" rx="1"/><rect x="2" y="17" width="20" height="4" rx="1"/><circle cx="18" cy="5" r="1" fill="currentColor"/><circle cx="18" cy="12" r="1" fill="currentColor"/><circle cx="18" cy="19" r="1" fill="currentColor"/></svg>
                {/if}
              </div>
              <div class="jb-name-col">
                <span class="jb-label">{s.name || s.ip}</span>
                <span class="jb-sub text-muted">{s.ip}</span>
              </div>
              <div class="jb-motd-col">
                {#if ping?.result?.motd_html}
                  <span class="jb-motd">{@html ping.result.motd_html}</span>
                {:else if ping?.loading}
                  <span class="text-muted" style="font-size:11px">…</span>
                {/if}
              </div>
              <div class="jb-right">
                {#if ping?.result?.online}
                  <span class="jb-players">
                    <svg viewBox="0 0 24 24" fill="var(--accent)" width="12" height="12"><path d="M12 12c2.7 0 4.8-2.1 4.8-4.8S14.7 2.4 12 2.4 7.2 4.5 7.2 7.2 9.3 12 12 12zm0 2.4c-3.2 0-9.6 1.6-9.6 4.8v2.4h19.2v-2.4c0-3.2-6.4-4.8-9.6-4.8z"/></svg>
                    {ping.result.players_online}
                  </span>
                {/if}
                <button class="btn btn-ghost btn-sm" on:click={() => launchJumpBack(s.instanceId, 'server', s.ip)} disabled={!activeAccount || !!launchingId}>
                  <svg viewBox="0 0 24 24" fill="currentColor" width="11" height="11"><polygon points="5 3 19 12 5 21 5 3"/></svg>
                  {$t('common.play')}
                </button>
                <button class="jb-dismiss" on:click={() => dismissEntry(`server:${s.instanceId}:${s.ip}`)} title={$t('common.remove')}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="12" height="12"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                </button>
              </div>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <!-- Recent Instances -->
    {#if recentInstances.length > 0}
      <section class="section">
        <div class="section-header">
          <span class="section-label">{$t('nav.instances')}</span>
          <button class="show-all-btn" on:click={() => currentPage.set('instances')}>{$t('home.allInstances')}</button>
        </div>
        <div class="recent-list">
          {#each recentInstances as inst (inst.id)}
            <div class="card-wrap">
              <InstanceCard
                {inst}
                {activeAccount}
                launchState={runningIds.includes(inst.id) ? { type: 'running' } : null}
                isLaunching={launchingId === inst.id}
                on:detail={() => openDetail(inst.id)}
                on:play={() => launchInstance(inst.id)}
                on:stop={() => stopInstance(inst.id)}
              />
            </div>
          {/each}
        </div>
      </section>
    {:else}
      <!-- Empty state -->
      <div class="empty-hero">
        <div class="empty-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="48" height="48"><rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>
        </div>
        <h2>{$t('home.noInstances')}</h2>
        <p class="text-muted">{$t('instances.emptyDesc')}</p>
        <button class="btn btn-primary" on:click={() => currentPage.set('instances')}>
          {$t('instances.createFirst')}
        </button>
      </div>
    {/if}

    <!-- Featured Modpacks -->
    <section class="section">
      <div class="section-header">
        <span class="section-label">{$t('home.featuredModpacks')}</span>
        <button class="show-all-btn" on:click={() => currentPage.set('modpacks')}>{$t('home.allModpacks')}</button>
      </div>

      {#if loadingModpacks}
        <div class="section-spinner">
          <NovaSpinner size={48} label={$t('versions.loading')} />
        </div>
      {:else if featuredModpacks.length > 0}
        <div class="modpack-grid">
          {#each featuredModpacks as pack}
            <button class="mp-result-card card card-hover" on:click={() => currentPage.set('modpacks')}>
              <div class="mp-card-banner">
                {#if pack.featured_gallery}
                  <img src={pack.featured_gallery} alt={pack.title} class="mp-card-banner-img" />
                {:else}
                  <div class="mp-card-banner-fallback" style="background: {pack.color ? `#${Math.abs(pack.color).toString(16).padStart(6,'0')}18` : 'var(--surface2)'}">
                    {#if pack.icon_url}
                      <img src={pack.icon_url} alt="" class="mp-banner-fallback-icon" />
                    {:else}
                      <span class="mp-banner-letter">{pack.title[0]}</span>
                    {/if}
                  </div>
                {/if}
              </div>
              {#if pack.icon_url && pack.featured_gallery}
                <img src={pack.icon_url} alt="" class="mp-card-icon-overlay" />
              {/if}
              <div class="mp-card-body" class:has-icon={!!(pack.icon_url && pack.featured_gallery)}>
                <div class="mp-card-title-wrap">
                  <span class="mp-card-title">{pack.title}</span>
                  <span class="mp-card-author text-muted">{pack.author || ''}</span>
                </div>
                <p class="mp-card-desc text-muted">{pack.description}</p>
                {#if pack.categories?.length > 0}
                  <div class="mp-card-tags">
                    {#each pack.categories.slice(0, 3) as cat}
                      <span class="badge badge-muted">{cat}</span>
                    {/each}
                    {#if pack.versions?.length > 0}
                      <span class="badge badge-info">{pack.versions[pack.versions.length - 1]}</span>
                    {/if}
                  </div>
                {/if}
                <div class="mp-card-footer">
                  <span class="mp-dl-count text-muted">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                    {formatDownloads(pack.downloads)}
                  </span>
                </div>
              </div>
            </button>
          {/each}
        </div>
      {:else}
        <p class="text-muted" style="font-size:13px">{$t('home.modpacksError')}</p>
      {/if}
    </section>

  </div>
</div>

{#if launchingId}
  {@const launchingInst = cfg?.instances?.find(i => i.id === launchingId)}
  <LaunchOverlay instanceName={launchingInst?.name ?? ''} />
{/if}

<style>
  .header-sub { font-size: 13px; margin-top: 3px; color: var(--text-muted); }

  /* Jump Back In */
  .jb-grid { display: flex; flex-direction: column; gap: 6px; }
  .jb-row {
    display: flex; align-items: center; gap: 12px; padding: 10px 14px;
    background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--radius); box-shadow: 0 1px 4px rgba(0,0,0,0.12);
    transition: all var(--transition);
  }
  .jb-row:hover { border-color: rgba(var(--accent-rgb),0.35); background: var(--surface2); box-shadow: 0 4px 14px rgba(0,0,0,0.22); transform: translateY(-1px); }
  .jb-server-row {
    display: grid; grid-template-columns: 40px 180px 1fr auto; align-items: center; gap: 12px; padding: 10px 14px;
    background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--radius); box-shadow: 0 1px 4px rgba(0,0,0,0.12);
    transition: all var(--transition);
  }
  .jb-server-row:hover { border-color: rgba(var(--accent-rgb),0.35); background: var(--surface2); box-shadow: 0 4px 14px rgba(0,0,0,0.22); transform: translateY(-1px); }
  .jb-icon { flex-shrink: 0; width: 40px; height: 40px; display: flex; align-items: center; justify-content: center; background: var(--surface2); border-radius: var(--radius-sm); border: 1px solid var(--border); }
  .jb-icon-img { width: 36px; height: 36px; object-fit: cover; border-radius: 4px; image-rendering: pixelated; }
  .jb-info { flex: 1; display: flex; flex-direction: column; gap: 2px; min-width: 0; overflow: hidden; }
  .jb-name-col { display: flex; flex-direction: column; gap: 2px; overflow: hidden; }
  .jb-motd-col { display: flex; align-items: center; justify-content: center; }
  .jb-right { display: flex; align-items: center; gap: 8px; justify-content: flex-end; }
  .jb-label { font-size: 13px; font-weight: 500; color: var(--text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .jb-sub { font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .jb-motd { font-family: 'Minecraft', monospace; font-size: 11px; text-align: center; white-space: pre-wrap; line-height: 1.6; word-break: break-word; }
  .jb-players { display: flex; align-items: center; gap: 3px; font-size: 12px; color: var(--success); flex-shrink: 0; white-space: nowrap; font-weight: 500; }
  .jb-dismiss {
    flex-shrink: 0; display: flex; align-items: center; justify-content: center;
    width: 22px; height: 22px; padding: 0;
    background: none; border: none; border-radius: var(--radius-sm);
    color: var(--text-muted); cursor: pointer; opacity: 0; transition: all var(--transition);
  }
  .jb-row:hover .jb-dismiss,
  .jb-server-row:hover .jb-dismiss { opacity: 1; }
  .jb-dismiss:hover { color: var(--danger, #e55); background: rgba(255,80,80,0.1); }

  .section { margin-bottom: 36px; }
  .section-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px; }
  .show-all-btn {
    font-size: 12px; color: var(--accent); background: rgba(var(--accent-rgb),0.06);
    border: 1px solid rgba(var(--accent-rgb),0.2); border-radius: var(--radius-sm);
    cursor: pointer; padding: 4px 12px; transition: all var(--transition); font-weight: 500;
  }
  .show-all-btn:hover { background: rgba(var(--accent-rgb),0.14); border-color: var(--accent); }

  /* Recent instances */
  .recent-list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 8px;
  }

  .card-wrap { display: flex; flex-direction: column; }

  /* Empty hero */
  .empty-hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    padding: 56px 24px;
    text-align: center;
    margin-bottom: 36px;
    background: linear-gradient(135deg, color-mix(in srgb, var(--accent) 4%, var(--surface)), var(--surface));
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }
  .empty-icon {
    width: 72px; height: 72px; border-radius: 50%;
    background: rgba(var(--accent-rgb),0.1); border: 1px solid rgba(var(--accent-rgb),0.2);
    display: flex; align-items: center; justify-content: center;
    color: var(--accent); margin-bottom: 4px;
  }
  .empty-hero h2 { font-size: 19px; font-weight: 700; }
  .empty-hero p { font-size: 14px; max-width: 360px; line-height: 1.5; }

  /* Modpack grid */
  .modpack-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 14px;
  }

  /* Card — mirrors Discover's result-card exactly */
  .mp-result-card {
    display: flex; flex-direction: column;
    overflow: visible; position: relative;
    cursor: pointer; text-align: left;
  }

  /* Banner */
  .mp-card-banner {
    height: 128px; overflow: hidden;
    background: var(--surface2);
    border-radius: var(--radius) var(--radius) 0 0;
    position: relative; display: flex;
    align-items: center; justify-content: center; flex-shrink: 0;
  }
  .mp-card-banner-img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .mp-card-banner-fallback { width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; }
  .mp-banner-fallback-icon { width: 64px; height: 64px; object-fit: contain; border-radius: 12px; opacity: 0.75; }
  .mp-banner-letter { font-size: 40px; font-weight: 800; color: var(--border); }

  /* Icon overlay */
  .mp-card-icon-overlay {
    position: absolute; top: calc(128px - 26px); left: 12px;
    width: 52px; height: 52px; border-radius: 10px;
    border: 2px solid var(--surface); object-fit: contain;
    image-rendering: pixelated; z-index: 3;
    background: var(--surface2); box-shadow: 0 2px 8px rgba(0,0,0,0.3);
  }

  /* Body */
  .mp-card-body { padding: 12px; display: flex; flex-direction: column; gap: 7px; flex: 1; }
  .mp-card-body.has-icon { padding-top: 34px; }

  .mp-card-title-wrap { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .mp-card-title { font-size: 13px; font-weight: 600; color: var(--text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .mp-card-author { font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  .mp-card-desc { font-size: 12px; line-height: 1.4; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }

  .mp-card-tags { display: flex; flex-wrap: wrap; gap: 4px; }

  .mp-card-footer { display: flex; align-items: center; justify-content: space-between; margin-top: auto; padding-top: 4px; }
  .mp-dl-count { display: flex; align-items: center; gap: 4px; font-size: 11px; }

  .section-spinner {
    display: flex;
    justify-content: center;
    padding: 40px 0;
  }
</style>
