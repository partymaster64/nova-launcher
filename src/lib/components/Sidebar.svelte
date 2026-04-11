<script>
  import { onMount, onDestroy } from 'svelte'
  import { invoke, Channel } from '@tauri-apps/api/core'
  import { config, accounts, currentPage, detailInstanceId, detailActiveTab, isOnline as isOnlineStore } from '../../store.js'
  import { t } from '../i18n.js'

  let cfg = null
  let accs = []
  let page = 'home'

  config.subscribe(v => (cfg = v))
  accounts.subscribe(v => (accs = v))
  currentPage.subscribe(v => (page = v))

  // Direct polling for running state
  let runningIds = []

  $: runningInstances = runningIds.map(id => ({
    id,
    name: cfg?.instances?.find(i => i.id === id)?.name || id,
  }))

  $: navItems = [
    { id: 'home',      label: $t('nav.home'),      icon: iconHome() },
    { id: 'instances', label: $t('nav.instances'), icon: iconInstances() },
    { id: 'modpacks',  label: $t('nav.discover'),  icon: iconDiscover() },
    { id: 'skins',     label: $t('nav.skins'),     icon: iconSkins() },
    { id: 'settings',  label: $t('nav.settings'),  icon: iconSettings() },
  ]

  function navigate(id) { currentPage.set(id) }

  $: activeAccount = accs.find(a => a.uuid === cfg?.active_account_uuid)

  // ── Account Popover ──────────────────────────────────────────────────────────
  let showPopover = false
  let loginState = 'idle' // 'idle' | 'opening' | 'waiting' | 'error'
  let loginError = ''
  let unlistens = []
  let loginPollIv = null // unused, kept für Kompatibilität
  let confettiCanvas = null
  let confettiActive = false

  function launchConfetti() {
    if (!confettiCanvas) return
    confettiActive = true
    const ctx = confettiCanvas.getContext('2d')
    confettiCanvas.width = window.innerWidth
    confettiCanvas.height = window.innerHeight

    const colors = ['#ff6b6b','#feca57','#48dbfb','#ff9ff3','#54a0ff','#5f27cd','#00d2d3','#1dd1a1']
    const pieces = Array.from({ length: 140 }, () => ({
      x: Math.random() * confettiCanvas.width,
      y: Math.random() * confettiCanvas.height - confettiCanvas.height,
      w: 8 + Math.random() * 8,
      h: 4 + Math.random() * 6,
      color: colors[Math.floor(Math.random() * colors.length)],
      rot: Math.random() * Math.PI * 2,
      rotSpeed: (Math.random() - 0.5) * 0.15,
      vx: (Math.random() - 0.5) * 3,
      vy: 2 + Math.random() * 4,
      opacity: 1,
    }))

    const start = performance.now()
    function frame(now) {
      if (!confettiActive) return
      const elapsed = now - start
      ctx.clearRect(0, 0, confettiCanvas.width, confettiCanvas.height)
      let alive = false
      for (const p of pieces) {
        p.x += p.vx
        p.y += p.vy
        p.rot += p.rotSpeed
        if (elapsed > 2000) p.opacity = Math.max(0, p.opacity - 0.012)
        if (p.opacity > 0 && p.y < confettiCanvas.height + 20) alive = true
        ctx.save()
        ctx.globalAlpha = p.opacity
        ctx.translate(p.x, p.y)
        ctx.rotate(p.rot)
        ctx.fillStyle = p.color
        ctx.fillRect(-p.w / 2, -p.h / 2, p.w, p.h)
        ctx.restore()
      }
      if (alive) requestAnimationFrame(frame)
      else { confettiActive = false; ctx.clearRect(0, 0, confettiCanvas.width, confettiCanvas.height) }
    }
    requestAnimationFrame(frame)
  }

  onMount(() => {
    const pollIv = setInterval(async () => {
      try { runningIds = await invoke('get_running_instances') } catch (_) {}
    }, 750)
    unlistens.push(() => clearInterval(pollIv))
  })

  onDestroy(() => { unlistens.forEach(u => u()) })

  function stopLoginPoll() {
    if (loginPollIv) { clearInterval(loginPollIv); loginPollIv = null }
  }

  async function startBrowserLogin() {
    loginState = 'opening'
    loginError = ''
    try {
      const channel = new Channel()
      channel.onmessage = async (msg) => {
        if (msg.type === 'complete') {
          try {
            await invoke('set_active_account', { uuid: msg.account.uuid })
            const [updatedCfg, latestAccs] = await Promise.all([
              invoke('get_config'),
              invoke('get_accounts'),
            ])
            config.set(updatedCfg)
            accounts.set(latestAccs)
          } catch (_) {}
          loginState = 'idle'
          launchConfetti()
        } else if (msg.type === 'error') {
          loginError = msg.message
          loginState = 'error'
        }
      }
      await invoke('start_login_browser', { onEvent: channel })
      loginState = 'waiting'
    } catch (e) {
      loginError = String(e)
      loginState = 'error'
    }
  }

  async function removeAccount(uuid) {
    try {
      await invoke('remove_account', { uuid })
      const [updatedCfg, updatedAccs] = await Promise.all([
        invoke('get_config'),
        invoke('get_accounts'),
      ])
      config.set(updatedCfg)
      accounts.set(updatedAccs)
    } catch (e) { console.error(e) }
  }

  async function setActiveAccount(uuid) {
    try {
      await invoke('set_active_account', { uuid })
      config.set(await invoke('get_config'))
    } catch (e) { console.error(e) }
  }

  // Online-Status: aus globalem Store lesen + pollt alle 3s
  let backOnlineFlash = false   // kurz grün anzeigen wenn Verbindung wiederkehrt
  let backOnlineTimer = null

  onMount(async () => {
    try {
      const initial = await invoke('get_online_status')
      isOnlineStore.set(initial)
    } catch (_) {}

    const iv = setInterval(async () => {
      try {
        const now = await invoke('get_online_status')
        isOnlineStore.update(prev => {
          if (!prev && now) {
            // War offline, jetzt online → kurzen Flash zeigen
            backOnlineFlash = true
            clearTimeout(backOnlineTimer)
            backOnlineTimer = setTimeout(() => { backOnlineFlash = false }, 3000)
          }
          return now
        })
      } catch (_) {}
    }, 3000)
    unlistens.push(() => clearInterval(iv))
  })

  function goToInstance(id) {
    detailInstanceId.set(id)
    detailActiveTab.set('content')
    currentPage.set('instance-detail')
  }

  function goToInstanceLogs(id) {
    detailInstanceId.set(id)
    detailActiveTab.set('logs')
    currentPage.set('instance-detail')
  }

  function togglePopover() {
    showPopover = !showPopover
    if (!showPopover) { loginState = 'idle'; stopLoginPoll() }
  }

  function closePopover() {
    showPopover = false
    loginState = 'idle'
    stopLoginPoll()
  }

  function iconHome() { return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>` }
  function iconDiscover() { return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polygon points="16.24 7.76 14.12 14.12 7.76 16.24 9.88 9.88 16.24 7.76"/></svg>` }
  function iconInstances() { return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>` }
  function iconSkins() { return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>` }
  function iconSettings() { return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>` }
</script>

<canvas
  bind:this={confettiCanvas}
  style="position:fixed;top:0;left:0;width:100vw;height:100vh;pointer-events:none;z-index:9999"
></canvas>

<aside class="sidebar">

  <!-- Logo -->
  <div class="logo">
    <div class="logo-icon">
      <svg viewBox="0 0 160 160" width="30" height="30" xmlns="http://www.w3.org/2000/svg">
        <rect x="4"  y="4"  width="152" height="152" rx="18" fill="var(--ni-1)"/>
        <rect x="16" y="16" width="128" height="128" rx="16" fill="var(--ni-2)"/>
        <rect x="28" y="28" width="104" height="104" rx="14" fill="var(--ni-3)"/>
        <rect x="40" y="40" width="80"  height="80"  rx="12" fill="var(--ni-4)"/>
        <rect x="52" y="52" width="56"  height="56"  rx="10" fill="var(--ni-5)"/>
        <rect x="64" y="64" width="32"  height="32"  rx="7"  fill="var(--ni-6)"/>
        <rect x="73" y="73" width="14"  height="14"  rx="4"  fill="var(--ni-7)"/>
      </svg>
    </div>
    <div class="logo-text-wrap">
      <span class="logo-name">Nova</span>
      <span class="logo-sub">Launcher</span>
    </div>
  </div>

  <!-- Connectivity status bar (under logo) -->
  {#if !$isOnlineStore}
    <div class="conn-bar conn-bar--offline">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11" style="flex-shrink:0"><line x1="1" y1="1" x2="23" y2="23"/><path d="M16.72 11.06A10.94 10.94 0 0 1 19 12.55M5 12.55a10.94 10.94 0 0 1 5.17-2.39M10.71 5.05A16 16 0 0 1 22.56 9M1.42 9a15.91 15.91 0 0 1 4.7-2.88M8.53 16.11a6 6 0 0 1 6.95 0M12 20h.01"/></svg>
      <span>Offline</span>
    </div>
  {:else if backOnlineFlash}
    <div class="conn-bar conn-bar--online">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="11" height="11" style="flex-shrink:0"><polyline points="20 6 9 17 4 12"/></svg>
      <span>Wieder online</span>
    </div>
  {/if}

  <!-- Nav -->
  <nav class="nav">
    {#each navItems as item}
      {@const isActive = page === item.id || (item.id === 'instances' && page === 'instance-detail') || (item.id === 'skins' && page === 'skins')}
      {@const offlineDisabled = !$isOnlineStore && (item.id === 'modpacks' || item.id === 'skins')}
      <button
        class="nav-item"
        class:active={isActive}
        class:nav-offline={offlineDisabled}
        on:click={() => navigate(item.id)}
        title={offlineDisabled ? 'Kein Internet' : ''}
      >
        {#if isActive}<span class="nav-indicator"></span>{/if}
        <span class="nav-icon">{@html item.icon}</span>
        <span class="nav-label">{item.label}</span>
        {#if offlineDisabled}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="10" height="10" style="margin-left:auto;flex-shrink:0;opacity:0.5"><line x1="1" y1="1" x2="23" y2="23"/><path d="M16.72 11.06A10.94 10.94 0 0 1 19 12.55M5 12.55a10.94 10.94 0 0 1 5.17-2.39M8.53 16.11a6 6 0 0 1 6.95 0M12 20h.01"/></svg>
        {/if}
      </button>
    {/each}
  </nav>

  <!-- Running Instances -->
  {#if runningInstances.length > 0}
    <div class="running-section">
      <div class="running-section-label">{$t('sidebar.running')}</div>
      {#each runningInstances as ri}
        <!-- svelte-ignore a11y-click-events-have-key-events -->
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div class="running-row" on:click={() => goToInstance(ri.id)}>
          <span class="running-dot"></span>
          <span class="running-name">{ri.name}</span>
          <button class="logs-btn" title={$t('sidebar.showLogs')} on:click|stopPropagation={() => goToInstanceLogs(ri.id)}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
              <polyline points="14 2 14 8 20 8"/>
              <line x1="16" y1="13" x2="8" y2="13"/>
              <line x1="16" y1="17" x2="8" y2="17"/>
            </svg>
          </button>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Bottom: Account -->
  <div class="sidebar-bottom">

    <!-- Account Popover -->
    {#if showPopover}
      <!-- svelte-ignore a11y-click-events-have-key-events -->
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div class="popover-backdrop" on:click={closePopover}></div>

      <div class="account-popover">
        <div class="popover-header">
          <span class="popover-title">{$t('sidebar.accounts')}</span>
          <button class="popover-close" on:click={closePopover}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>

        {#if accs.length > 0}
          <div class="popover-accounts">
            {#each accs as acc (acc.uuid)}
              {@const isActive = cfg?.active_account_uuid === acc.uuid}
              <div class="popover-account" class:is-active={isActive}>
                <div class="popover-avatar-wrap">
                  <img
                    src={`https://mc-heads.net/avatar/${acc.uuid.replace(/-/g,'')}/28`}
                    alt={acc.username}
                    class="popover-avatar-img"
                    on:error={e => { e.target.style.display='none'; e.target.nextElementSibling.style.display='flex' }}
                  />
                  <!-- svelte-ignore a11y-aria-attributes-misused -->
                  <span class="popover-avatar-fallback" style="display:none">{acc.username[0].toUpperCase()}</span>
                  {#if isActive}<span class="active-dot"></span>{/if}
                </div>
                <span class="popover-username" class:active-name={isActive}>{acc.username}</span>
                <div class="popover-account-actions">
                  {#if !isActive}
                    <button class="icon-btn switch-btn" title={$t('sidebar.switchAccount')} on:click={() => setActiveAccount(acc.uuid)}>
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="13" height="13"><polyline points="9 18 15 12 9 6"/></svg>
                    </button>
                  {/if}
                  <button class="icon-btn logout-btn" title={$t('sidebar.logout')} on:click={() => removeAccount(acc.uuid)}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/></svg>
                  </button>
                </div>
              </div>
            {/each}
          </div>
        {:else}
          <p class="popover-empty">{$t('sidebar.noAccount')}</p>
        {/if}

        <div class="popover-footer">
          {#if loginState === 'idle'}
            <button class="add-account-btn" on:click={startBrowserLogin}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="12" height="12"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
              {$t('sidebar.addAccount')}
            </button>
          {:else if loginState === 'opening' || loginState === 'waiting'}
            <div class="login-progress">
              <div class="login-spinner"></div>
              <span class="login-state-text">
                {loginState === 'opening' ? $t('sidebar.browserOpening') : $t('sidebar.waitingLogin')}
              </span>
              <button class="icon-btn" title={$t('sidebar.cancelLogin')} on:click={() => { loginState = 'idle'; stopLoginPoll() }}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="12" height="12"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
              </button>
            </div>
          {:else if loginState === 'error'}
            <div class="login-error">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13" style="color:var(--error);flex-shrink:0"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>
              <span class="error-text">{loginError}</span>
              <button class="retry-btn" on:click={startBrowserLogin}>{$t('common.retry')}</button>
            </div>
          {/if}
        </div>
      </div>
    {/if}

    <!-- Account trigger -->
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div class="account-trigger" class:no-account={!activeAccount} class:open={showPopover} on:click={togglePopover}>
      <div class="account-avatar">
        {#if activeAccount}
          <img
            src={`https://mc-heads.net/avatar/${activeAccount.uuid.replace(/-/g,'')}/32`}
            alt={activeAccount.username}
            class="avatar-img"
            on:error={e => { e.target.style.display='none'; e.target.nextElementSibling.style.display='flex' }}
          />
          <span class="avatar-fallback" style="display:none">{activeAccount.username[0].toUpperCase()}</span>
          <span class="avatar-online"></span>
        {:else}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="15" height="15" style="color:var(--text-muted)"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
        {/if}
      </div>
      <div class="account-details">
        {#if activeAccount}
          <span class="account-name">{activeAccount.username}</span>
          <span class="account-status" class:offline-status={!$isOnlineStore}>
            {$isOnlineStore ? $t('sidebar.loggedIn') : $t('sidebar.offlineMode')}
          </span>
        {:else}
          <span class="account-name no-acc">{$t('sidebar.notLoggedIn')}</span>
          <span class="account-status dim">{$t('sidebar.clickToLogin')}</span>
        {/if}
      </div>
      <svg class="chevron" class:rotated={showPopover} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11"><polyline points="18 15 12 9 6 15"/></svg>
    </div>
  </div>
</aside>

<style>
  .sidebar {
    width: var(--sidebar-width); min-width: var(--sidebar-width);
    background: var(--surface); border-right: 1px solid var(--border);
    display: flex; flex-direction: column; height: 100vh; overflow: hidden;
  }

  /* ── Logo ── */
  .logo {
    display: flex; align-items: center; gap: 11px;
    padding: 0 16px;
    height: var(--topbar-h);
    box-sizing: border-box;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .logo-icon {
    width: 36px; height: 36px; display: flex; align-items: center; justify-content: center;
    flex-shrink: 0;
    --ni-1: color-mix(in srgb, var(--accent) 18%, black);
    --ni-2: color-mix(in srgb, var(--accent) 32%, black);
    --ni-3: color-mix(in srgb, var(--accent) 52%, black);
    --ni-4: var(--accent);
    --ni-5: color-mix(in srgb, var(--accent) 60%, white);
    --ni-6: color-mix(in srgb, var(--accent) 35%, white);
    --ni-7: color-mix(in srgb, var(--accent) 14%, white);
  }
  .logo-text-wrap { display: flex; flex-direction: column; gap: 0; line-height: 1.1; }
  .logo-name { font-size: 15px; font-weight: 800; color: var(--accent); letter-spacing: -0.01em; }
  .logo-sub { font-size: 9px; font-weight: 600; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.12em; }

  /* ── Nav ── */
  .nav { flex: 1; padding: 10px 8px; display: flex; flex-direction: column; gap: 1px; overflow-y: auto; }

  .nav-item {
    position: relative;
    display: flex; align-items: center; gap: 10px;
    width: 100%; padding: 9px 12px;
    border-radius: var(--radius-sm);
    font-size: 13px; font-weight: 500;
    color: var(--text-dim);
    cursor: pointer; transition: all var(--transition);
    border: none; background: none; text-align: left;
    overflow: hidden;
  }
  .nav-item:hover { background: var(--surface2); color: var(--text); }
  .nav-item.active { background: rgba(var(--accent-rgb),0.1); color: var(--accent); font-weight: 600; }
  .nav-item.active:hover { background: rgba(var(--accent-rgb),0.15); }
  .nav-item.nav-offline { opacity: 0.45; cursor: not-allowed; }
  .nav-item.nav-offline:hover { background: none; color: var(--text-dim); }

  /* Left accent bar for active state */
  .nav-indicator {
    position: absolute; left: 0; top: 20%; bottom: 20%;
    width: 3px; border-radius: 0 3px 3px 0;
    background: var(--accent);
  }

  .nav-icon { width: 16px; height: 16px; display: flex; align-items: center; justify-content: center; flex-shrink: 0; }
  .nav-icon :global(svg) { width: 16px; height: 16px; }
  .nav-label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  /* ── Running Instances ── */
  .running-section { padding: 6px 8px; border-top: 1px solid var(--border); }
  .running-section-label { font-size: 9px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.1em; color: var(--text-muted); padding: 4px 4px 6px; }
  .running-row { display: flex; align-items: center; gap: 7px; padding: 5px 6px; border-radius: var(--radius-sm); cursor: pointer; transition: background var(--transition); }
  .running-row:hover { background: var(--surface2); }

  .running-dot {
    width: 7px; height: 7px; border-radius: 50%; flex-shrink: 0;
    background: var(--success);
  }

  .running-name { flex: 1; font-size: 11px; color: var(--text-dim); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .logs-btn { width: 20px; height: 20px; display: flex; align-items: center; justify-content: center; border-radius: 4px; color: var(--text-muted); flex-shrink: 0; transition: all var(--transition); background: none; border: none; cursor: pointer; padding: 0; }
  .logs-btn:hover { background: var(--surface3); color: var(--accent); }

  /* ── Sidebar Bottom ── */
  .sidebar-bottom { padding: 8px; border-top: 1px solid var(--border); position: relative; }

  /* ── Account Trigger ── */
  .account-trigger {
    display: flex; align-items: center; gap: 10px;
    padding: 8px; border-radius: var(--radius-sm);
    background: var(--surface2); border: 1px solid var(--border);
    cursor: pointer; transition: all var(--transition); user-select: none;
  }
  .account-trigger:hover { border-color: rgba(var(--accent-rgb),0.4); background: rgba(var(--accent-rgb),0.05); }
  .account-trigger.open { border-color: var(--accent); background: rgba(var(--accent-rgb),0.08); }

  .account-avatar {
    position: relative;
    width: 30px; height: 30px; border-radius: 6px;
    background: var(--surface3);
    display: flex; align-items: center; justify-content: center;
    flex-shrink: 0; border: 1px solid var(--border); overflow: visible;
  }
  .avatar-img { width: 30px; height: 30px; border-radius: 5px; object-fit: cover; display: block; image-rendering: pixelated; }
  .avatar-fallback { width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; font-size: 13px; font-weight: 700; color: var(--accent); border-radius: 5px; }

  /* Online indicator dot */
  .avatar-online {
    position: absolute; bottom: -2px; right: -2px;
    width: 9px; height: 9px; border-radius: 50%;
    background: var(--success); border: 2px solid var(--surface2);
  }

  .account-details { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .account-name { font-size: 12px; font-weight: 600; color: var(--text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .account-name.no-acc { color: var(--text-muted); font-weight: 500; }
  .account-status { font-size: 10px; color: var(--success); }
  .account-status.dim { color: var(--text-muted); }

  .chevron { flex-shrink: 0; color: var(--text-muted); transition: transform var(--transition); }
  .chevron.rotated { transform: rotate(180deg); }

  /* ── Popover Backdrop ── */
  .popover-backdrop { position: fixed; inset: 0; z-index: 99; }

  /* ── Account Popover ── */
  .account-popover {
    position: absolute; bottom: calc(100% + 6px); left: 0; right: 0;
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius);
    box-shadow: 0 -8px 32px rgba(0,0,0,0.5); z-index: 100; overflow: hidden;
  }

  .popover-header { display: flex; align-items: center; justify-content: space-between; padding: 10px 12px 8px; border-bottom: 1px solid var(--border); }
  .popover-title { font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.1em; color: var(--text-muted); }
  .popover-close { width: 22px; height: 22px; border-radius: 50%; display: flex; align-items: center; justify-content: center; color: var(--text-muted); transition: all var(--transition); background: none; border: none; cursor: pointer; padding: 0; }
  .popover-close:hover { background: var(--surface2); color: var(--text); }

  .popover-accounts { display: flex; flex-direction: column; padding: 4px; gap: 1px; }

  .popover-account { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: var(--radius-sm); transition: background var(--transition); }
  .popover-account:hover { background: var(--surface2); }
  .popover-account.is-active { background: rgba(var(--accent-rgb),0.07); }

  .popover-avatar-wrap { position: relative; width: 28px; height: 28px; flex-shrink: 0; }
  .popover-avatar-img { width: 28px; height: 28px; border-radius: 3px; object-fit: cover; image-rendering: pixelated; display: block; }
  .popover-avatar-fallback { width: 28px; height: 28px; border-radius: 3px; background: var(--surface3); display: flex; align-items: center; justify-content: center; font-size: 12px; font-weight: 700; color: var(--accent); border: 1px solid var(--border); }
  .active-dot { position: absolute; bottom: -2px; right: -2px; width: 8px; height: 8px; border-radius: 50%; background: var(--success); border: 2px solid var(--surface); }

  .popover-username { flex: 1; font-size: 12px; font-weight: 500; color: var(--text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .popover-username.active-name { color: var(--accent); font-weight: 600; }

  .popover-account-actions { display: flex; gap: 2px; flex-shrink: 0; }

  .icon-btn { width: 24px; height: 24px; border-radius: var(--radius-sm); display: flex; align-items: center; justify-content: center; color: var(--text-muted); transition: all var(--transition); background: none; border: none; cursor: pointer; padding: 0; }
  .icon-btn:hover { background: var(--surface3); color: var(--text); }
  .switch-btn:hover { color: var(--accent); }
  .logout-btn:hover { color: var(--error); background: rgba(248,113,113,0.1); }

  .popover-empty { font-size: 12px; color: var(--text-muted); text-align: center; padding: 14px 12px; }

  /* ── Popover Footer ── */
  .popover-footer { padding: 4px; border-top: 1px solid var(--border); }

  .add-account-btn { display: flex; align-items: center; gap: 6px; width: 100%; padding: 7px 10px; border-radius: var(--radius-sm); font-size: 12px; font-weight: 500; color: var(--accent); background: rgba(var(--accent-rgb),0.07); border: 1px dashed rgba(var(--accent-rgb),0.3); transition: all var(--transition); cursor: pointer; }
  .add-account-btn:hover { background: rgba(var(--accent-rgb),0.14); border-color: var(--accent); border-style: solid; }

  .login-progress { display: flex; align-items: center; gap: 8px; padding: 7px 8px; }
  .login-spinner { width: 12px; height: 12px; border: 1.5px solid var(--border); border-top-color: var(--accent); border-radius: 50%; animation: spin 0.7s linear infinite; flex-shrink: 0; }
  .login-state-text { flex: 1; font-size: 11px; color: var(--text-dim); }
  @keyframes spin { to { transform: rotate(360deg) } }

  .login-error { display: flex; align-items: center; gap: 6px; padding: 7px 8px; }
  .error-text { flex: 1; font-size: 11px; color: var(--error); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .retry-btn { font-size: 11px; color: var(--accent); padding: 2px 6px; border-radius: var(--radius-sm); border: 1px solid rgba(var(--accent-rgb),0.3); cursor: pointer; white-space: nowrap; transition: all var(--transition); background: none; }
  .retry-btn:hover { background: rgba(var(--accent-rgb),0.1); }

  /* ── Connectivity status bar ── */
  .conn-bar {
    display: flex; align-items: center; justify-content: center; gap: 5px;
    padding: 4px 12px;
    font-size: 11px; font-weight: 600; letter-spacing: 0.03em;
    animation: conn-bar-in 0.25s ease;
  }
  @keyframes conn-bar-in {
    from { opacity: 0; transform: translateY(-4px); }
    to   { opacity: 1; transform: translateY(0); }
  }
  .conn-bar--offline {
    background: rgba(251,146,60,0.15); border-bottom: 1px solid rgba(251,146,60,0.3);
    color: #fb923c;
  }
  .conn-bar--online {
    background: rgba(34,197,94,0.12); border-bottom: 1px solid rgba(34,197,94,0.25);
    color: #22c55e;
    animation: conn-bar-in 0.25s ease, conn-bar-out 0.4s ease 2.6s forwards;
  }
  @keyframes conn-bar-out {
    to { opacity: 0; transform: translateY(-4px); }
  }

  .offline-status { color: var(--text-muted) !important; }
</style>
