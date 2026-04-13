<script>
  import { onMount } from 'svelte'
  import { get } from 'svelte/store'
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'

  import Toast from './lib/components/Toast.svelte'
  import Sidebar from './lib/components/Sidebar.svelte'
  import Home from './lib/pages/Home.svelte'
  import Instances from './lib/pages/Instances.svelte'
  import Discover from './lib/pages/Discover.svelte'
  import Settings from './lib/pages/Settings.svelte'
  import InstanceDetail from './lib/pages/InstanceDetail.svelte'
  import SkinManager from './lib/pages/SkinManager.svelte'
  import SetupWizard from './lib/pages/SetupWizard.svelte'
  import CrashModal from './lib/components/CrashModal.svelte'
  import UpdateModal from './lib/components/UpdateModal.svelte'
  import { check as checkUpdate } from '@tauri-apps/plugin-updater'

  import { config, accounts, manifest, currentPage, instanceLaunchStates, instanceLogs, instanceUpdates, crashEvent, showSetupWizard } from './store.js'

  let page = 'home'
  let pendingUpdate = null
  let cfg = null
  config.subscribe(v => { cfg = v })
  currentPage.subscribe(v => (page = v))

  function _accentHexFromFloat(r, g, b) {
    const toHex = v => Math.round(v * 255).toString(16).padStart(2, '0')
    return `#${toHex(r)}${toHex(g)}${toHex(b)}`
  }
  function _accentLighten(hex, amount) {
    const p = (_h, s, e) => parseInt(hex.slice(s, e), 16) / 255
    const [r, g, b] = [p(hex,1,3), p(hex,3,5), p(hex,5,7)]
    const c = v => Math.min(1, v + amount)
    const toH = v => Math.round(v * 255).toString(16).padStart(2, '0')
    return `#${toH(c(r))}${toH(c(g))}${toH(c(b))}`
  }
  function _accentDarken(hex, amount) {
    const p = (_h, s, e) => parseInt(hex.slice(s, e), 16) / 255
    const [r, g, b] = [p(hex,1,3), p(hex,3,5), p(hex,5,7)]
    const c = v => Math.max(0, v - amount)
    const toH = v => Math.round(v * 255).toString(16).padStart(2, '0')
    return `#${toH(c(r))}${toH(c(g))}${toH(c(b))}`
  }

  function applyStartupTheme(cfg) {
    if (!cfg) return
    const root = document.documentElement
    const themes = {
      '':         { bg: '#0d0d14', surface: '#16151f', surface2: '#1e1c2a', border: '#2e2a45', text: '#f0eaff', textDim: '#9d8fc0', textMuted: '#5a5075' },
      'oled':     { bg: '#000000', surface: '#090909', surface2: '#111111', border: '#1e1e1e', text: '#ffffff', textDim: '#aaaaaa', textMuted: '#555555' },
      'soft':     { bg: '#111118', surface: '#1a1928', surface2: '#211f32', border: '#302d4a', text: '#ede8ff', textDim: '#9890be', textMuted: '#5a5480' },
      'midnight': { bg: '#07070f', surface: '#0e0d1a', surface2: '#161528', border: '#252340', text: '#e8e0ff', textDim: '#8880b8', textMuted: '#484068' },
      'forest':   { bg: '#0c1410', surface: '#131c18', surface2: '#1b2622', border: '#263830', text: '#e6f4ec', textDim: '#86b097', textMuted: '#4a6855' },
      'ocean':    { bg: '#090f1a', surface: '#101828', surface2: '#172030', border: '#1e3048', text: '#deeeff', textDim: '#7898cc', textMuted: '#3a5878' },
      'warm':     { bg: '#13100d', surface: '#1e1814', surface2: '#28201b', border: '#3a2e26', text: '#f5ece4', textDim: '#b0958a', textMuted: '#6a5048' },
      'light':    { bg: '#f0edf8', surface: '#faf8ff', surface2: '#e8e4f4', border: '#d0c8e8', text: '#1a1530', textDim: '#4a4570', textMuted: '#8a85b0' },
    }
    const t = themes[cfg.ui_theme || ''] || themes['']
    root.style.setProperty('--bg', t.bg)
    root.style.setProperty('--surface', t.surface)
    root.style.setProperty('--surface2', t.surface2)
    root.style.setProperty('--border', t.border)
    root.style.setProperty('--text', t.text)
    root.style.setProperty('--text-dim', t.textDim)
    root.style.setProperty('--text-muted', t.textMuted)
    // Accent — also derive hover/dim/rgb so ALL rgba() usages update
    if (cfg.accent_color) {
      const hex = _accentHexFromFloat(cfg.accent_color.r, cfg.accent_color.g, cfg.accent_color.b)
      const ri = Math.round(cfg.accent_color.r * 255)
      const gi = Math.round(cfg.accent_color.g * 255)
      const bi = Math.round(cfg.accent_color.b * 255)
      root.style.setProperty('--accent', hex)
      root.style.setProperty('--accent-hover', _accentLighten(hex, 0.15))
      root.style.setProperty('--accent-dim',   _accentDarken(hex, 0.3))
      root.style.setProperty('--accent-rgb', `${ri}, ${gi}, ${bi}`)
    }
    // Radius
    const radiusMap = { '': ['8px','4px','12px'], 'compact': ['3px','2px','6px'], 'rounded': ['14px','8px','20px'], 'pill': ['999px','999px','999px'] }
    const [r, rsm, rlg] = radiusMap[cfg.ui_radius || ''] || radiusMap['']
    root.style.setProperty('--radius', r)
    root.style.setProperty('--radius-sm', rsm)
    root.style.setProperty('--radius-lg', rlg)
    // Font family
    const fontStacks = {
      '':            "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
      'nunito':      "'Nunito', sans-serif",
      'oxanium':     "'Oxanium', sans-serif",
      'merriweather':"'Merriweather', Georgia, serif",
      'mono':        "'JetBrains Mono', 'Cascadia Code', monospace",
    }
    root.style.setProperty('--font', fontStacks[cfg.ui_font || ''] || fontStacks[''])
    // UI scale (zoom)
    document.getElementById('app').style.zoom = cfg.ui_scale || 1.0
    // Font size, density, sidebar width
    root.style.fontSize = `${cfg.ui_font_size || 14}px`
    if (cfg.ui_sidebar_width) root.style.setProperty('--sidebar-width', `${cfg.ui_sidebar_width}px`)
    const pad = cfg.ui_density === 'compact' ? '0.7' : cfg.ui_density === 'comfortable' ? '1.35' : '1'
    root.style.setProperty('--density', pad)
    if (cfg.ui_animations === false) root.style.setProperty('--transition', '0s')
    const isLight = cfg.ui_theme === 'light'
    root.style.setProperty('--success', isLight ? '#059669' : '#34d399')
    root.style.setProperty('--warning', isLight ? '#d97706' : '#fbbf24')
    root.style.setProperty('--error',   isLight ? '#dc2626' : '#f87171')
    root.style.setProperty('--info',    isLight ? '#2563eb' : '#60a5fa')
  }

  onMount(async () => {
    document.addEventListener('contextmenu', e => e.preventDefault())
    try {
      const [cfg, accs] = await Promise.all([
        invoke('get_config'),
        invoke('get_accounts'),
      ])
      config.set(cfg)
      accounts.set(accs)
      invoke('get_manifest').then(m => manifest.set(m)).catch(() => {})
      // Apply saved theme on startup
      applyStartupTheme(cfg)
      // Show setup wizard if not completed yet
      if (!cfg.setup_complete) {
        showSetupWizard.set(true)
      }
      // Check for app updates in the background (only in production builds)
      if (!import.meta.env.DEV) {
        setTimeout(async () => {
          try {
            const update = await checkUpdate()
            if (update) pendingUpdate = update
          } catch (_) {}
        }, 3000)
      }
    } catch (e) {
      console.error('Init error:', e)
    } finally {
      // Close the native splashscreen window and reveal the main window
      invoke('close_splashscreen').catch(() => {})
    }

    // ── Crash detection: unconditional 1s poll ───────────────────────────────
    // Works regardless of whether cfg is loaded or events were dropped (e.g. WebKitGTK
    // freezes JS when the window is hidden via minimize_on_launch).
    const crashPollInterval = setInterval(async () => {
      if (get(crashEvent)) return
      try {
        const pending = await invoke('get_pending_crash')
        if (pending && !get(crashEvent)) {
          const [instanceId, error] = pending
          const instanceName = cfg?.instances?.find(i => i.id === instanceId)?.name ?? instanceId
          crashEvent.set({ instanceId, instanceName, error })
        }
      } catch (_) {}
    }, 1000)

    // ── Background update check (startup + periodic) ────────────────────────
    function pollInstanceUpdates() {
      if (!cfg?.instances) return
      for (const inst of cfg.instances) {
        // First load cached result immediately, then trigger fresh check
        invoke('get_instance_updates', { instanceId: inst.id })
          .then(cached => { if (cached != null) instanceUpdates.update(u => ({ ...u, [inst.id]: cached })) })
          .catch(() => {})
        invoke('check_instance_updates', { instanceId: inst.id })
          .then(updates => instanceUpdates.update(u => ({ ...u, [inst.id]: updates })))
          .catch(() => {})
      }
    }
    // Delay first check slightly so the UI loads first
    setTimeout(pollInstanceUpdates, 8000)
    // Re-check every 30 minutes
    setInterval(pollInstanceUpdates, 30 * 60 * 1000)

    // ── Event listener (progress / token_refreshed) ──────────────────────────
    const unlisten = await listen('launch_event', (event) => {
      const payload = event.payload
      const iid = payload.instance_id ?? payload.instanceId
      const etype = payload.event_type ?? payload.eventType
      if (iid) {
        instanceLaunchStates.update(states => {
          const next = { ...states }
          switch (etype) {
            case 'progress':
              next[iid] = { type: 'downloading', step: payload.step, percent: payload.percent }
              break
            case 'done':
              delete next[iid]
              invoke('get_config').then(c => config.set(c)).catch(() => {})
              break
            case 'error':
              next[iid] = { type: 'error', error: payload.error }
              break
          }
          return next
        })
        if (etype === 'done' || etype === 'error') {
          instanceLogs.update(logs => { const n = { ...logs }; delete n[iid]; return n })
        }
      }

      if (etype === 'token_refreshed') {
        invoke('get_accounts').then(accs => accounts.set(accs)).catch(() => {})
      }

      // Show crash modal — fast path when Tauri events are delivered normally
      if (etype === 'error' && payload.error && iid) {
        const name = cfg?.instances?.find(i => i.id === iid)?.name ?? iid
        crashEvent.set({ instanceId: iid, instanceName: name, error: payload.error })
      }
    })

    // ── Polling: running state + logs (every 750ms) ───────────────────────────
    const pollInterval = setInterval(async () => {
      try {
        const running = await invoke('get_running_instances')
        const runningSet = new Set(running)

        instanceLaunchStates.update(states => {
          const next = { ...states }
          for (const iid of running) {
            if (!next[iid] || next[iid].type === 'downloading') {
              next[iid] = { type: 'running' }
            }
          }
          for (const [iid, s] of Object.entries(next)) {
            if (s.type === 'running' && !runningSet.has(iid)) {
              delete next[iid]
              invoke('get_config').then(c => config.set(c)).catch(() => {})
            }
          }
          return next
        })

        for (const iid of running) {
          invoke('get_instance_logs', { instanceId: iid }).then(lines => {
            instanceLogs.update(logs => ({ ...logs, [iid]: lines }))
          }).catch(() => {})
        }
      } catch (_) {}
    }, 750)

    return () => { unlisten(); clearInterval(pollInterval); clearInterval(crashPollInterval) }
  })

  async function onSetupComplete() {
    showSetupWizard.set(false)
    // Re-fetch config to get setup_complete = true and apply new theme/accent
    try {
      const updatedCfg = await invoke('get_config')
      config.set(updatedCfg)
      applyStartupTheme(updatedCfg)
    } catch (_) {}
  }

  async function onSetupRestartComplete() {
    currentPage.set('settings')
    try {
      const updatedCfg = await invoke('get_config')
      config.set(updatedCfg)
      applyStartupTheme(updatedCfg)
    } catch (_) {}
  }
</script>

{#if $showSetupWizard}
  <SetupWizard on:complete={onSetupComplete} />
{/if}

<Toast />

{#if $crashEvent}
  <CrashModal instanceId={$crashEvent.instanceId} instanceName={$crashEvent.instanceName} error={$crashEvent.error} />
{/if}

{#if pendingUpdate}
  <UpdateModal update={pendingUpdate} on:close={() => pendingUpdate = null} />
{/if}

{#if page === 'setup'}
  <SetupWizard isRestart={true} on:complete={onSetupRestartComplete} on:close={() => currentPage.set('settings')} />
{:else}
  <div class="app-layout">
    <Sidebar />
    <main class="main-content">
      {#if page === 'home'}
        <Home />
      {:else if page === 'instances'}
        <Instances />
      {:else if page === 'instance-detail'}
        <InstanceDetail />
      {:else if page === 'modpacks'}
        <div class="page-wrap"><Discover /></div>
      {:else if page === 'skins'}
        <SkinManager />
      {:else if page === 'settings'}
        <Settings />
      {/if}
    </main>
  </div>
{/if}

<style>
  @import url('https://fonts.googleapis.com/css2?family=Nunito:wght@400;500;600;700&family=Oxanium:wght@400;500;600;700&family=Merriweather:wght@400;700&family=JetBrains+Mono:wght@400;500;700&display=swap');

  @font-face {
    font-family: 'Minecraft';
    src: url('https://cdn.jsdelivr.net/gh/South-Paw/typeface-minecraft@master/files/minecraft.woff2') format('woff2'),
         url('https://cdn.jsdelivr.net/gh/South-Paw/typeface-minecraft@master/files/minecraft.woff') format('woff');
    font-weight: normal;
    font-style: normal;
  }
  :global(*) { margin: 0; padding: 0; box-sizing: border-box; }
  :global(:root) {
    --bg: #0d0d14; --surface: #16151f; --surface2: #1e1c2a; --surface3: #252336;
    --border: #2e2a45; --accent: #a855f7; --accent-dim: #6b21a8; --accent-hover: #c084fc; --accent-rgb: 168, 85, 247;
    --text: #f0eaff; --text-dim: #9d8fc0; --text-muted: #5a5075;
    --success: #34d399; --warning: #fbbf24; --error: #f87171; --info: #60a5fa;
    --radius: 8px; --radius-sm: 4px; --radius-lg: 12px; --transition: 0.15s ease;
    --sidebar-width: 220px; --density: 1; --topbar-h: 71px;
    --font: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Inter', sans-serif;
    --shadow-sm: 0 1px 3px rgba(0,0,0,0.2), 0 1px 2px rgba(0,0,0,0.1);
    --shadow: 0 4px 12px rgba(0,0,0,0.25), 0 2px 4px rgba(0,0,0,0.12);
    --shadow-lg: 0 12px 40px rgba(0,0,0,0.45), 0 4px 12px rgba(0,0,0,0.2);
    font-size: 14px;
  }
  :global(input), :global(textarea) { -webkit-user-select: text; user-select: text; }
  :global(body) {
    background: var(--bg); color: var(--text);
    font-family: var(--font);
    font-size: 1rem; line-height: 1.5; overflow: hidden; height: 100vh;
  }
  :global(#app) { height: 100vh; overflow: hidden; }
  :global(*) { scrollbar-width: thin; scrollbar-color: var(--border) transparent; }
  :global(::-webkit-scrollbar) { width: 6px; height: 6px; }
  :global(::-webkit-scrollbar-track) { background: transparent; }
  :global(::-webkit-scrollbar-thumb) { background: var(--border); border-radius: 3px; }
  :global(::-webkit-scrollbar-thumb:hover) { background: var(--text-muted); }
  /* Dropdown list: fixed-position overlay needs explicit track color */
  :global(.sel-list::-webkit-scrollbar) { width: 6px; }
  :global(.sel-list::-webkit-scrollbar-track) { background: var(--surface2); }
  :global(.sel-list::-webkit-scrollbar-thumb) { background: var(--border); border-radius: 3px; }
  :global(.sel-list::-webkit-scrollbar-thumb:hover) { background: var(--text-muted); }
  :global(button) { cursor: pointer; border: none; background: none; font-family: inherit; font-size: inherit; color: inherit; transition: all var(--transition); }
  :global(input) { font-family: inherit; font-size: inherit; }
  :global(a) { color: var(--accent); text-decoration: none; }
  :global(a:hover) { color: var(--accent-hover); }
  :global(.btn) { display: inline-flex; align-items: center; gap: 6px; padding: calc(0.571rem * var(--density)) calc(1.143rem * var(--density)); border-radius: var(--radius-sm); font-size: 0.928rem; font-weight: 500; cursor: pointer; transition: all var(--transition); border: 1px solid transparent; white-space: nowrap; outline: none; }
  :global(button) { outline: none; }
  :global(.btn-primary) { background: var(--accent); color: #fff; border-color: var(--accent); }
  :global(.btn-primary:hover:not(:disabled)) { background: var(--accent-hover); border-color: var(--accent-hover); box-shadow: 0 0 12px rgba(var(--accent-rgb),0.3); }
  :global(.btn-ghost) { background: transparent; color: var(--text-dim); border-color: var(--border); }
  :global(.btn-ghost:hover:not(:disabled)) { background: rgba(var(--accent-rgb),0.08); color: var(--accent); border-color: rgba(var(--accent-rgb),0.35); }
  :global(.btn-danger) { background: transparent; color: var(--error); border-color: rgba(248,113,113,0.3); }
  :global(.btn-danger:hover:not(:disabled)) { background: rgba(248,113,113,0.1); border-color: var(--error); }
  :global(.btn-sm) { padding: calc(0.357rem * var(--density)) calc(0.714rem * var(--density)); font-size: 0.857rem; }
  :global(.btn:disabled) { opacity: 0.45; cursor: not-allowed; }
  :global(.card) { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); transition: all var(--transition); box-shadow: 0 1px 4px rgba(0,0,0,0.15); }
  :global(.card-hover:hover) { transform: translateY(-2px); box-shadow: 0 8px 24px rgba(0,0,0,0.3), 0 2px 8px rgba(0,0,0,0.15); border-color: rgba(var(--accent-rgb),0.4); }
  :global(.input) { background: var(--surface2); border: 1px solid var(--border); border-radius: var(--radius-sm); color: var(--text); padding: calc(0.571rem * var(--density)) calc(0.857rem * var(--density)); width: 100%; outline: none; transition: border-color var(--transition); font-size: 0.928rem; }
  :global(.input:focus) { border-color: var(--accent); }
  :global(.badge) { display: inline-flex; align-items: center; padding: 2px 8px; border-radius: 100px; font-size: 0.786rem; font-weight: 500; }
  :global(.badge-accent) { background: rgba(var(--accent-rgb),0.15); color: var(--accent); border: 1px solid rgba(var(--accent-rgb),0.3); }
  :global(.badge-success) { background: rgba(52,211,153,0.12); color: var(--success); border: 1px solid rgba(52,211,153,0.25); }
  :global(.badge-warning) { background: rgba(251,191,36,0.12); color: var(--warning); border: 1px solid rgba(251,191,36,0.25); }
  :global(.badge-muted) { background: var(--surface2); color: var(--text-muted); border: 1px solid var(--border); }
  :global(.badge-info) { background: rgba(96,165,250,0.12); color: var(--info); border: 1px solid rgba(96,165,250,0.25); }
  :global(.page) { height: 100%; display: flex; flex-direction: column; overflow: hidden; }
  :global(.page-header) { display: flex; align-items: center; justify-content: space-between; height: var(--topbar-h); padding: 0 1.714rem; border-bottom: 1px solid var(--border); flex-shrink: 0; box-sizing: border-box; background: linear-gradient(to bottom, color-mix(in srgb, var(--accent) 5%, var(--surface)), var(--surface)); }
  :global(.page-title) { font-size: 1.286rem; font-weight: 700; color: var(--text); }
  :global(.page-body) { flex: 1; overflow-y: auto; padding: calc(1.429rem * var(--density)) 1.714rem; }
  :global(.divider) { border: none; border-top: 1px solid var(--border); margin: 12px 0; }
  :global(.text-accent) { color: var(--accent); } :global(.text-dim) { color: var(--text-dim); }
  :global(.text-muted) { color: var(--text-muted); } :global(.text-success) { color: var(--success); }
  :global(.text-warning) { color: var(--warning); } :global(.text-error) { color: var(--error); }
  :global(.section-label) { font-size: 0.786rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.08em; color: var(--text-muted); margin-bottom: 12px; display: inline-block; padding-left: 9px; border-left: 2px solid var(--accent); }
.app-layout { display: flex; height: 100vh; overflow: hidden; }
  .main-content { flex: 1; min-width: 0; overflow: hidden; display: flex; flex-direction: column; }
  .page-wrap { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
</style>
