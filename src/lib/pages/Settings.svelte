<script>
  import { onMount, onDestroy, tick } from 'svelte'
  import { get } from 'svelte/store'
  import { invoke } from '@tauri-apps/api/core'
  import { config, currentPage, addToast } from '../../store.js'
  import { t } from '../i18n.js'

  let cfg = null
  let initialized = false
  let saveTimeout = null
  let saveStatus = '' // '' | 'saving' | 'saved' | 'error'
  let saveError = ''
  let activeTab = 'general'
  let legalModal = '' // '' | 'impressum' | 'privacy'

  // ── General ──
  let language = 'de'
  let minimizeOnLaunch = true

  // ── Improvements ──
  let autoCompressScreenshots = false
  let screenshotSync = false
  let savesSync = false
  let serversSync = false
  let configSync = false
  let resourcePackSync = false

  // ── Discord RPC ──
  let discordRpcEnabled = false
  onDestroy(() => {})

  // ── App folder & Cache ──
  let novaDir = ''
  let cacheSize = 0
  let cacheClearing = false
  let changingDir = false

  // ── Global RAM ──
  let globalRamMax = 2048
  let globalJavaPath = ''
  let systemRamMb = 8192

  // ── Java Management ──
  const JAVA_VERSIONS = [
    { ver: '26', label: 'Java 26', mc: 'Aktuell (März 2026) · kein LTS' },
    { ver: '25', label: 'Java 25', mc: 'LTS (Sep 2025)' },
    { ver: '21', label: 'Java 21', mc: 'LTS · MC 1.20.5 – 26.x' },
    { ver: '17', label: 'Java 17', mc: 'LTS · MC 1.17 – 1.20.4' },
    { ver: '8',  label: 'Java 8',  mc: 'LTS · MC 1.0 – 1.16' },
  ]
  let javaPath8 = ''
  let javaPath17 = ''
  let javaPath21 = ''
  let javaPath25 = ''
  let javaPath26 = ''
  let javaOp = { '8': null, '17': null, '21': null, '25': null, '26': null } // null | 'test' | 'detect' | 'install'
  let javaStatus = { '8': null, '17': null, '21': null, '25': null, '26': null } // null | { ok, text }

  // ── Default window ──
  let defaultWidth = 854
  let defaultHeight = 480
  let defaultFullscreen = false

  // ── Default JVM / hooks ──
  let defaultCustomJvmArgs = ''
  let defaultEnvVars = [['']]
  let defaultPreLaunchHook = ''
  let defaultWrapperCommand = ''
  let defaultPostExitHook = ''

  // ── Version filters ──
  let showSnapshots = false
  let showOldAlpha = false
  let showOldBeta = false

  // ── Appearance ──
  let accentHex = '#a855f7'
  let uiTheme = ''
  let uiRadius = ''
  let uiFontSize = 14
  let uiFont = ''
  let uiScale = 1.0
  let uiDensity = ''
  let uiAnimations = true
  let uiSidebarWidth = 220

  const languages = [
    { value: 'de', label: 'Deutsch' },
    { value: 'en', label: 'English' },
    { value: 'fr', label: 'Français' },
  ]

  const colorPresets = [
    { key: 'colorPurple',  hex: '#a855f7' },
    { key: 'colorViolet',  hex: '#8b5cf6' },
    { key: 'colorIndigo',  hex: '#6366f1' },
    { key: 'colorBlue',    hex: '#3b82f6' },
    { key: 'colorCyan',    hex: '#06b6d4' },
    { key: 'colorTeal',    hex: '#14b8a6' },
    { key: 'colorGreen',   hex: '#10b981' },
    { key: 'colorLime',    hex: '#84cc16' },
    { key: 'colorYellow',  hex: '#f59e0b' },
    { key: 'colorOrange',  hex: '#f97316' },
    { key: 'colorRed',     hex: '#ef4444' },
    { key: 'colorPink',    hex: '#ec4899' },
    { key: 'colorRose',    hex: '#fb7185' },
    { key: 'colorWhite',   hex: '#e2e8f0' },
    { key: 'colorSilver',  hex: '#94a3b8' },
    { key: 'colorGold',    hex: '#d4a017' },
  ]

  const themes = [
    { id: '',         label: 'Dark',     bg: '#0d0d14', surface: '#16151f', surface2: '#1e1c2a', border: '#2e2a45', text: '#f0eaff', textDim: '#9d8fc0', textMuted: '#5a5075' },
    { id: 'oled',     label: 'OLED',     bg: '#000000', surface: '#090909', surface2: '#111111', border: '#1e1e1e', text: '#ffffff', textDim: '#aaaaaa', textMuted: '#555555' },
    { id: 'soft',     label: 'Cozy',     bg: '#111118', surface: '#1a1928', surface2: '#211f32', border: '#302d4a', text: '#ede8ff', textDim: '#9890be', textMuted: '#5a5480' },
    { id: 'midnight', label: 'Midnight', bg: '#07070f', surface: '#0e0d1a', surface2: '#161528', border: '#252340', text: '#e8e0ff', textDim: '#8880b8', textMuted: '#484068' },
    { id: 'forest',   label: 'Forest',   bg: '#0c1410', surface: '#131c18', surface2: '#1b2622', border: '#263830', text: '#e6f4ec', textDim: '#86b097', textMuted: '#4a6855' },
    { id: 'ocean',    label: 'Ocean',    bg: '#090f1a', surface: '#101828', surface2: '#172030', border: '#1e3048', text: '#deeeff', textDim: '#7898cc', textMuted: '#3a5878' },
    { id: 'warm',     label: 'Warm',     bg: '#13100d', surface: '#1e1814', surface2: '#28201b', border: '#3a2e26', text: '#f5ece4', textDim: '#b0958a', textMuted: '#6a5048' },
    { id: 'light',    label: 'Light',    bg: '#f0edf8', surface: '#faf8ff', surface2: '#e8e4f4', border: '#d0c8e8', text: '#1a1530', textDim: '#4a4570', textMuted: '#8a85b0' },
  ]

  const radii = [
    { id: '',         labelKey: 'settings.radiusStandard', preview: '8px' },
    { id: 'compact',  labelKey: 'settings.radiusCompact',  preview: '3px' },
    { id: 'rounded',  labelKey: 'settings.radiusRounded',  preview: '14px' },
  ]

  const fonts = [
    { id: '',             labelKey: 'settings.fontStandard',     stack: "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif" },
    { id: 'nunito',       labelKey: 'settings.fontNunito',        stack: "'Nunito', sans-serif" },
    { id: 'oxanium',      labelKey: 'settings.fontOxanium',       stack: "'Oxanium', sans-serif" },
    { id: 'merriweather', labelKey: 'settings.fontMerriweather',  stack: "'Merriweather', Georgia, serif" },
    { id: 'mono',         labelKey: 'settings.fontMono',          stack: "'JetBrains Mono', monospace" },
  ]

  const uiScales = [
    { value: 0.8,  label: 'XS' },
    { value: 0.9,  label: 'S' },
    { value: 1.0,  label: 'M' },
    { value: 1.1,  label: 'L' },
    { value: 1.2,  label: 'XL' },
  ]

  const densities = [
    { id: '',            labelKey: 'settings.densityNormal',      descKey: 'settings.densityNormalDesc' },
    { id: 'compact',     labelKey: 'settings.densityCompact',     descKey: 'settings.densityCompactDesc' },
    { id: 'comfortable', labelKey: 'settings.densityComfortable', descKey: 'settings.densityComfortableDesc' },
  ]

  const sidebarWidths = [
    { value: 180, labelKey: 'settings.sidebarNarrow' },
    { value: 220, labelKey: 'settings.sidebarNormal' },
    { value: 260, labelKey: 'settings.sidebarWide' },
  ]

  const windowPresets = [
    { label: '854×480',   w: 854,  h: 480  },
    { label: '1280×720',  w: 1280, h: 720  },
    { label: '1920×1080', w: 1920, h: 1080 },
  ]

  const RAM_SNAPS = [512, 1024, 2048, 4096, 6144, 8192, 12288, 16384, 24576, 32768]

  function ramSnapPoints(maxMb) {
    const tiers = [2048, 4096, 8192, 16384, 32768, 65536]
    const rounded = tiers.find(t => t >= maxMb) ?? maxMb
    return RAM_SNAPS.filter(v => v <= Math.max(rounded, 2048))
  }

  $: ramSnaps = ramSnapPoints(systemRamMb)

  $: ramMaxIdx = (() => {
    let best = 0
    for (let i = 0; i < ramSnaps.length; i++) {
      if (ramSnaps[i] <= globalRamMax) best = i
    }
    return best
  })()

  function onRamMaxSlider(e) {
    globalRamMax = ramSnaps[parseInt(e.target.value)]
  }

  function addEnvVar() { defaultEnvVars = [...defaultEnvVars, ['', '']] }
  function removeEnvVar(i) { defaultEnvVars = defaultEnvVars.filter((_, idx) => idx !== i) }

  function getJavaPath(ver) {
    if (ver === '8') return javaPath8
    if (ver === '17') return javaPath17
    if (ver === '21') return javaPath21
    if (ver === '25') return javaPath25
    return javaPath26
  }
  function setJavaPath(ver, val) {
    if (ver === '8') javaPath8 = val
    else if (ver === '17') javaPath17 = val
    else if (ver === '21') javaPath21 = val
    else if (ver === '25') javaPath25 = val
    else javaPath26 = val
  }

  async function testJava(ver) {
    const path = getJavaPath(ver)
    if (!path.trim()) return
    javaOp = { ...javaOp, [ver]: 'test' }
    javaStatus = { ...javaStatus, [ver]: null }
    try {
      await invoke('test_java', { path })
      addToast(`Java ${ver} funktioniert`, 'success')
    } catch (e) {
      javaStatus = { ...javaStatus, [ver]: { ok: false, text: String(e) } }
    }
    javaOp = { ...javaOp, [ver]: null }
  }

  async function detectJava(ver) {
    javaOp = { ...javaOp, [ver]: 'detect' }
    javaStatus = { ...javaStatus, [ver]: null }
    try {
      const path = await invoke('detect_java', { version: parseInt(ver) })
      if (path) {
        setJavaPath(ver, path)
        addToast(`Java ${ver} gefunden`, 'success')
      } else {
        javaStatus = { ...javaStatus, [ver]: { ok: false, text: $t('settings.javaNotFound') } }
      }
    } catch (e) {
      javaStatus = { ...javaStatus, [ver]: { ok: false, text: String(e) } }
    }
    javaOp = { ...javaOp, [ver]: null }
  }

  async function installJava(ver) {
    javaOp = { ...javaOp, [ver]: 'install' }
    javaStatus = { ...javaStatus, [ver]: null }
    try {
      const path = await invoke('install_java', { version: parseInt(ver) })
      setJavaPath(ver, path)
      addToast(`Java ${ver} installiert`, 'success')
    } catch (e) {
      javaStatus = { ...javaStatus, [ver]: { ok: false, text: String(e) } }
    }
    javaOp = { ...javaOp, [ver]: null }
  }

  function formatBytes(b) {
    if (b >= 1073741824) return `${(b / 1073741824).toFixed(1)} GB`
    if (b >= 1048576) return `${(b / 1048576).toFixed(1)} MB`
    if (b >= 1024) return `${(b / 1024).toFixed(0)} KB`
    return `${b} B`
  }

  function formatRam(mb) {
    if (mb >= 1024) return `${(mb / 1024).toFixed(mb % 1024 === 0 ? 0 : 1)} GB`
    return `${mb} MB`
  }

  async function doChangeAppDir() {
    changingDir = true
    try {
      const selected = await invoke('pick_folder')
      if (!selected) return
      await invoke('change_app_dir', { newPath: selected })
      novaDir = selected
    } catch (e) {
      console.error(e)
    } finally {
      changingDir = false
    }
  }

  function restartSetup() {
    currentPage.set('setup')
  }

  async function doClearCache() {
    cacheClearing = true
    try {
      await invoke('clear_cache')
      cacheSize = 0
    } catch (e) {
      console.error(e)
    } finally {
      cacheClearing = false
    }
  }

  onMount(async () => {
    cfg = get(config)
    if (cfg) {
      language = cfg.language || 'de'
      minimizeOnLaunch = cfg.minimize_on_launch !== false
      globalRamMax = cfg.global_ram_max_mb || 2048
      globalJavaPath = cfg.global_java_path || ''
      javaPath8  = cfg.java_paths?.['8']  || ''
      javaPath17 = cfg.java_paths?.['17'] || ''
      javaPath21 = cfg.java_paths?.['21'] || ''
      javaPath25 = cfg.java_paths?.['25'] || ''
      javaPath26 = cfg.java_paths?.['26'] || ''
      defaultWidth = cfg.default_game_width || 854
      defaultHeight = cfg.default_game_height || 480
      defaultFullscreen = cfg.default_fullscreen || false
      defaultCustomJvmArgs = cfg.default_custom_jvm_args || ''
      defaultEnvVars = cfg.default_env_vars?.length ? cfg.default_env_vars : [['', '']]
      defaultPreLaunchHook = cfg.default_pre_launch_hook || ''
      defaultWrapperCommand = cfg.default_wrapper_command || ''
      defaultPostExitHook = cfg.default_post_exit_hook || ''
      showSnapshots = cfg.show_snapshots || false
      showOldAlpha = cfg.show_old_alpha || false
      showOldBeta = cfg.show_old_beta || false
      discordRpcEnabled = cfg.discord_rpc_enabled || false
      autoCompressScreenshots = cfg.auto_compress_screenshots || false
      screenshotSync = cfg.screenshot_sync || false
      savesSync = cfg.saves_sync || false
      serversSync = cfg.servers_sync || false
      configSync = cfg.config_sync || false
      resourcePackSync = cfg.resource_pack_sync || false
      uiTheme = cfg.ui_theme || ''
      uiRadius = cfg.ui_radius || ''
      uiFontSize = cfg.ui_font_size || 14
      uiDensity = cfg.ui_density || ''
      uiFont = cfg.ui_font || ''
      uiScale = cfg.ui_scale ?? 1.0
      uiAnimations = cfg.ui_animations !== false
      uiSidebarWidth = cfg.ui_sidebar_width || 220
      if (cfg.accent_color) {
        const r = Math.round(cfg.accent_color.r * 255).toString(16).padStart(2, '0')
        const g = Math.round(cfg.accent_color.g * 255).toString(16).padStart(2, '0')
        const b = Math.round(cfg.accent_color.b * 255).toString(16).padStart(2, '0')
        accentHex = `#${r}${g}${b}`
      }
      applyTheme(uiTheme, accentHex, uiRadius, uiFontSize, uiFont, uiScale, uiDensity, uiAnimations, uiSidebarWidth)
    }
    try {
      novaDir = await invoke('get_nova_dir')
      systemRamMb = await invoke('get_system_ram_mb')
      cacheSize = await invoke('get_cache_size')
    } catch (_) {}
    await tick()
    initialized = true
  })

  $: if (initialized) scheduleSave(
    language, minimizeOnLaunch,
    globalRamMax, globalJavaPath,
    defaultWidth, defaultHeight, defaultFullscreen,
    defaultCustomJvmArgs, defaultEnvVars,
    defaultPreLaunchHook, defaultWrapperCommand, defaultPostExitHook,
    showSnapshots, showOldAlpha, showOldBeta,
    discordRpcEnabled,
    accentHex, uiTheme, uiRadius, uiFontSize, uiFont, uiScale, uiDensity, uiAnimations, uiSidebarWidth,
    autoCompressScreenshots, screenshotSync, savesSync, serversSync, configSync, resourcePackSync,
    javaPath8, javaPath17, javaPath21, javaPath25,
  )

  function scheduleSave(..._) {
    clearTimeout(saveTimeout)
    saveStatus = ''
    saveTimeout = setTimeout(doSave, 700)
  }

  async function doSave() {
    if (!cfg) return
    saveStatus = 'saving'
    try {
      const [r, g, b] = hexToRgbFloat(accentHex)
      const cleanEnvVars = defaultEnvVars.filter(row => row[0]?.trim())
      const updated = {
        ...cfg,
        language,
        close_on_launch: false,
        minimize_on_launch: minimizeOnLaunch,
        global_ram_min_mb: 512,
        global_ram_max_mb: globalRamMax,
        global_java_path: globalJavaPath || null,
        default_game_width: defaultWidth,
        default_game_height: defaultHeight,
        default_fullscreen: defaultFullscreen,
        default_custom_jvm_args: defaultCustomJvmArgs || null,
        default_env_vars: cleanEnvVars.length ? cleanEnvVars : null,
        default_pre_launch_hook: defaultPreLaunchHook || null,
        default_wrapper_command: defaultWrapperCommand || null,
        default_post_exit_hook: defaultPostExitHook || null,
        show_snapshots: showSnapshots,
        show_old_alpha: showOldAlpha,
        show_old_beta: showOldBeta,
        discord_rpc_enabled: discordRpcEnabled,
        accent_color: { r, g, b },
        ui_theme: uiTheme,
        ui_radius: uiRadius,
        ui_font_size: uiFontSize,
        ui_font: uiFont,
        ui_scale: uiScale,
        ui_density: uiDensity,
        ui_animations: uiAnimations,
        ui_sidebar_width: uiSidebarWidth,
        auto_compress_screenshots: autoCompressScreenshots,
        screenshot_sync: screenshotSync,
        saves_sync: savesSync,
        servers_sync: serversSync,
        config_sync: configSync,
        resource_pack_sync: resourcePackSync,
        java_paths: { '8': javaPath8, '17': javaPath17, '21': javaPath21, '25': javaPath25, '26': javaPath26 },
      }
      await invoke('save_config', { config: updated })
      config.set(updated)
      cfg = updated
      applyTheme(uiTheme, accentHex, uiRadius, uiFontSize, uiFont, uiScale, uiDensity, uiAnimations, uiSidebarWidth)
      // Reload Discord RPC task whenever discord settings change
      invoke('reload_discord_rpc').catch(() => {})
      saveStatus = 'saved'
      setTimeout(() => { if (saveStatus === 'saved') saveStatus = '' }, 2000)
    } catch (e) {
      saveStatus = 'error'
      saveError = String(e)
    }
  }

  function applyTheme(theme, hex, radius, fontSize, font, scale, density, animations, sidebarWidth) {
    const root = document.documentElement
    const t = themes.find(t => t.id === theme) || themes[0]
    root.style.setProperty('--accent', hex)
    root.style.setProperty('--accent-hover', lighten(hex, 0.15))
    root.style.setProperty('--accent-dim', darken(hex, 0.3))
    root.style.setProperty('--accent-rgb', `${parseInt(hex.slice(1,3),16)}, ${parseInt(hex.slice(3,5),16)}, ${parseInt(hex.slice(5,7),16)}`)
    root.style.setProperty('--bg', t.bg)
    root.style.setProperty('--surface', t.surface)
    root.style.setProperty('--surface2', t.surface2)
    root.style.setProperty('--surface3', blendColors(t.surface, t.surface2, 0.5))
    root.style.setProperty('--border', t.border)
    root.style.setProperty('--text', t.text)
    root.style.setProperty('--text-dim', t.textDim)
    root.style.setProperty('--text-muted', t.textMuted)
    const isLight = theme === 'light'
    root.style.setProperty('--success', isLight ? '#059669' : '#34d399')
    root.style.setProperty('--warning', isLight ? '#d97706' : '#fbbf24')
    root.style.setProperty('--error',   isLight ? '#dc2626' : '#f87171')
    root.style.setProperty('--info',    isLight ? '#2563eb' : '#60a5fa')
    if (radius === 'compact') {
      root.style.setProperty('--radius', '3px')
      root.style.setProperty('--radius-sm', '2px')
      root.style.setProperty('--radius-lg', '6px')
    } else if (radius === 'rounded') {
      root.style.setProperty('--radius', '14px')
      root.style.setProperty('--radius-sm', '8px')
      root.style.setProperty('--radius-lg', '20px')
    } else {
      root.style.setProperty('--radius', '8px')
      root.style.setProperty('--radius-sm', '4px')
      root.style.setProperty('--radius-lg', '12px')
    }
    const fontStacks = {
      '':            "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
      'nunito':      "'Nunito', sans-serif",
      'oxanium':     "'Oxanium', sans-serif",
      'merriweather':"'Merriweather', Georgia, serif",
      'mono':        "'JetBrains Mono', 'Cascadia Code', monospace",
    }
    root.style.setProperty('--font', fontStacks[font || ''] || fontStacks[''])
    document.getElementById('app').style.zoom = scale || 1.0
    root.style.fontSize = `${fontSize || 14}px`
    const pad = density === 'compact' ? '0.7' : density === 'comfortable' ? '1.35' : '1'
    root.style.setProperty('--density', pad)
    root.style.setProperty('--transition', animations ? '0.15s ease' : '0s')
    root.style.setProperty('--sidebar-width', `${sidebarWidth || 220}px`)
  }

  function hexToRgbFloat(hex) {
    return [
      parseInt(hex.slice(1,3),16)/255,
      parseInt(hex.slice(3,5),16)/255,
      parseInt(hex.slice(5,7),16)/255,
    ]
  }

  function lighten(hex, amount) {
    const [r,g,b] = hexToRgbFloat(hex)
    const c = v => Math.min(1, v + amount)
    return `#${toHex(c(r))}${toHex(c(g))}${toHex(c(b))}`
  }

  function darken(hex, amount) {
    const [r,g,b] = hexToRgbFloat(hex)
    const c = v => Math.max(0, v - amount)
    return `#${toHex(c(r))}${toHex(c(g))}${toHex(c(b))}`
  }

  function blendColors(hex1, hex2, t) {
    const [r1,g1,b1] = hexToRgbFloat(hex1)
    const [r2,g2,b2] = hexToRgbFloat(hex2)
    const lerp = (a,b) => a + (b-a)*t
    return `#${toHex(lerp(r1,r2))}${toHex(lerp(g1,g2))}${toHex(lerp(b1,b2))}`
  }

  function toHex(v) { return Math.round(v * 255).toString(16).padStart(2, '0') }

  $: if (initialized && accentHex) {
    const root = document.documentElement
    root.style.setProperty('--accent', accentHex)
    root.style.setProperty('--accent-hover', lighten(accentHex, 0.15))
    root.style.setProperty('--accent-dim', darken(accentHex, 0.3))
    root.style.setProperty('--accent-rgb', `${parseInt(accentHex.slice(1,3),16)}, ${parseInt(accentHex.slice(3,5),16)}, ${parseInt(accentHex.slice(5,7),16)}`)
  }
</script>

<div class="page">
  <div class="page-header">
    <h1 class="page-title">{$t('settings.title')}</h1>
    <div class="save-indicator">
      {#if saveStatus === 'saving'}
        <span class="status-saving"><div class="dot-spin"></div>{$t('settings.saving')}</span>
      {:else if saveStatus === 'saved'}
        <span class="status-saved">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="13" height="13"><polyline points="20 6 9 17 4 12"/></svg>
          {$t('settings.saved')}
        </span>
      {:else if saveStatus === 'error'}
        <span class="status-error" title={saveError}>{$t('settings.saveError')}</span>
      {/if}
    </div>
  </div>

  <div class="tabs">
    <button class="tab" class:active={activeTab === 'general'}      on:click={() => (activeTab = 'general')}>{$t('settings.tabGeneral')}</button>
    <button class="tab" class:active={activeTab === 'appearance'}   on:click={() => (activeTab = 'appearance')}>{$t('settings.tabAppearance')}</button>
    <button class="tab" class:active={activeTab === 'instances'}    on:click={() => (activeTab = 'instances')}>{$t('settings.tabInstances')}</button>
    <button class="tab" class:active={activeTab === 'improvements'} on:click={() => (activeTab = 'improvements')}>{$t('settings.tabImprovements')}</button>
    <button class="tab" class:active={activeTab === 'java'} on:click={() => (activeTab = 'java')}>{$t('settings.tabJava')}</button>
  </div>

  <div class="page-body">
    {#if !cfg}
      <div class="loading-cfg text-muted"><div class="dot-spin"></div>{$t('common.loading')}</div>
    {:else}

      <!-- ── Allgemein ── -->
      {#if activeTab === 'general'}

        <!-- Language -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.language')}</div>
          <div class="lang-row">
            {#each languages as lang}
              <button
                class="lang-chip"
                class:active={language === lang.value}
                on:click={() => (language = lang.value)}
              >
                {lang.label}
              </button>
            {/each}
          </div>
        </div>

        <!-- Verhalten beim Start -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.behavior')}</div>
          <!-- svelte-ignore a11y-label-has-associated-control -->
          <label class="toggle-row">
            <div class="toggle-text">
              <span class="toggle-title">{$t('settings.minimizeOnLaunch')}</span>
              <span class="toggle-desc text-muted">{$t('settings.minimizeOnLaunchDesc')}</span>
            </div>
            <div class="toggle-switch" class:on={minimizeOnLaunch}
              on:click={() => (minimizeOnLaunch = !minimizeOnLaunch)}
              role="switch" aria-checked={minimizeOnLaunch} tabindex="0"
              on:keydown={e => e.key === ' ' && (minimizeOnLaunch = !minimizeOnLaunch)}>
              <div class="toggle-thumb"></div>
            </div>
          </label>
        </div>

        <!-- Discord RPC -->
        <div class="card settings-card rpc-card">
          <div class="rpc-header">
            <div class="rpc-icon-wrap">
              <svg viewBox="0 0 24 24" fill="currentColor" width="20" height="20"><path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057c.002.022.015.043.033.054a19.9 19.9 0 0 0 5.993 3.03.077.077 0 0 0 .084-.028 14.09 14.09 0 0 0 1.226-1.994.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03z"/></svg>
            </div>
            <div class="rpc-title-block">
              <span class="card-section-title" style="margin:0">{$t('settings.discord')}</span>
              <span class="text-muted" style="font-size:12px">{$t('settings.discordDesc')}</span>
            </div>
            <div class="toggle-switch" class:on={discordRpcEnabled}
              on:click={() => (discordRpcEnabled = !discordRpcEnabled)}
              role="switch" aria-checked={discordRpcEnabled} tabindex="0"
              on:keydown={e => e.key === ' ' && (discordRpcEnabled = !discordRpcEnabled)}>
              <div class="toggle-thumb"></div>
            </div>
          </div>

        </div>

        <!-- App Ordner -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.appFolder')}</div>
          <p class="hint-text text-muted">{$t('settings.appFolderDesc')}</p>
          <div class="folder-row">
            <div class="folder-path text-muted">{novaDir || '...'}</div>
            <button class="btn btn-ghost btn-sm" on:click={doChangeAppDir} disabled={changingDir}>
              {#if changingDir}
                <div class="dot-spin" style="width:10px;height:10px;border-width:1.5px"></div>
              {:else}
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
              {/if}
              {$t('settings.appFolderChange')}
            </button>
          </div>
          <p class="hint-text text-muted" style="margin-top:-4px">{$t('settings.appFolderHint')}</p>
        </div>

        <!-- Cache -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.cache')}</div>
          <div class="cache-row">
            <div class="cache-info">
              <span class="toggle-title">{$t('settings.cacheData')}</span>
              <span class="toggle-desc text-muted">
                {#if cacheSize > 0}{formatBytes(cacheSize)}{:else}{$t('settings.cacheNone')}{/if}
              </span>
            </div>
            <button class="btn btn-ghost btn-sm" on:click={doClearCache} disabled={cacheClearing}>
              {#if cacheClearing}
                <div class="dot-spin" style="width:10px;height:10px;border-width:1.5px"></div>
              {/if}
              {$t('settings.cacheClear')}
            </button>
          </div>
        </div>

        <!-- Versionen -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.versionFilter')}</div>
          <p class="hint-text text-muted">{$t('settings.versionFilterDesc')}</p>
          <div class="toggle-list">
            <!-- svelte-ignore a11y-label-has-associated-control -->
            <label class="toggle-row">
              <div class="toggle-text">
                <span class="toggle-title">{$t('settings.showSnapshots')}</span>
                <span class="toggle-desc text-muted">{$t('settings.showSnapshotsDesc')}</span>
              </div>
              <div class="toggle-switch" class:on={showSnapshots}
                on:click={() => (showSnapshots = !showSnapshots)}
                role="switch" aria-checked={showSnapshots} tabindex="0"
                on:keydown={e => e.key === ' ' && (showSnapshots = !showSnapshots)}>
                <div class="toggle-thumb"></div>
              </div>
            </label>
            <!-- svelte-ignore a11y-label-has-associated-control -->
            <label class="toggle-row">
              <div class="toggle-text">
                <span class="toggle-title">{$t('settings.showAlpha')}</span>
                <span class="toggle-desc text-muted">{$t('settings.showAlphaDesc')}</span>
              </div>
              <div class="toggle-switch" class:on={showOldAlpha}
                on:click={() => (showOldAlpha = !showOldAlpha)}
                role="switch" aria-checked={showOldAlpha} tabindex="0"
                on:keydown={e => e.key === ' ' && (showOldAlpha = !showOldAlpha)}>
                <div class="toggle-thumb"></div>
              </div>
            </label>
            <!-- svelte-ignore a11y-label-has-associated-control -->
            <label class="toggle-row" style="border-bottom:none">
              <div class="toggle-text">
                <span class="toggle-title">{$t('settings.showBeta')}</span>
                <span class="toggle-desc text-muted">{$t('settings.showBetaDesc')}</span>
              </div>
              <div class="toggle-switch" class:on={showOldBeta}
                on:click={() => (showOldBeta = !showOldBeta)}
                role="switch" aria-checked={showOldBeta} tabindex="0"
                on:keydown={e => e.key === ' ' && (showOldBeta = !showOldBeta)}>
                <div class="toggle-thumb"></div>
              </div>
            </label>
          </div>
        </div>

        <!-- Setup-Assistent -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.restartSetup')}</div>
          <div class="cache-row">
            <div class="cache-info">
              <span class="toggle-desc text-muted">{$t('settings.restartSetupDesc')}</span>
            </div>
            <button class="btn btn-ghost btn-sm" on:click={restartSetup}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="12" height="12"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 .49-4.95"/></svg>
              {$t('settings.restartSetup')}
            </button>
          </div>
        </div>

        <div class="app-footer">
          <span>Nova Launcher</span>
          <span class="footer-sep">·</span>
          <span>v0.1.0</span>
          <span class="footer-sep">·</span>
          <button class="footer-link" on:click={() => (legalModal = 'privacy')}>{$t('settings.privacy')}</button>
          <span class="footer-sep">·</span>
          <button class="footer-link" on:click={() => (legalModal = 'impressum')}>{$t('settings.impressum')}</button>
        </div>

      <!-- ── Erscheinungsbild ── -->
      {:else if activeTab === 'appearance'}

        <!-- Accent Color -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.accentColor')}</div>
          <div class="color-picker-row">
            <div class="color-picker-wrap">
              <input type="color" bind:value={accentHex} class="native-color-input" />
              <div class="color-swatch-big" style="background: {accentHex}">
                <svg viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" width="16" height="16" opacity=".8"><path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>
              </div>
            </div>
            <div class="color-info">
              <span class="color-hex-label">{accentHex.toUpperCase()}</span>
              <span class="hint-text text-muted">{$t('settings.colorPickerHint')}</span>
              <div class="accent-preview">
                <div class="ap-btn" style="background: {accentHex}; border-color: {accentHex}">Button</div>
                <div class="ap-tag" style="color: {accentHex}; background: {accentHex}18; border-color: {accentHex}40">Tag</div>
                <div class="ap-dot" style="background: {accentHex}"></div>
              </div>
            </div>
          </div>
          <div class="color-presets-grid">
            {#each colorPresets as preset}
              <button
                class="preset-swatch"
                class:active={accentHex.toLowerCase() === preset.hex.toLowerCase()}
                style="background: {preset.hex}; --ring: {preset.hex}"
                title={$t('settings.' + preset.key)}
                on:click={() => (accentHex = preset.hex)}
              ></button>
            {/each}
          </div>
        </div>

        <!-- Theme -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.theme')}</div>
          <div class="theme-grid">
            {#each themes as t}
              <button class="theme-card" class:selected={uiTheme === t.id} on:click={() => (uiTheme = t.id)}>
                <div class="theme-preview">
                  <div class="tp-bg" style="background: {t.bg}">
                    <div class="tp-sidebar" style="background: {t.surface}">
                      <div class="tp-nav-item" style="background: {accentHex}22; border-left: 2px solid {accentHex}"></div>
                      <div class="tp-nav-item2" style="background: {t.surface2}"></div>
                      <div class="tp-nav-item2" style="background: {t.surface2}"></div>
                    </div>
                    <div class="tp-main">
                      <div class="tp-bar" style="background: {t.surface}; border-bottom: 1px solid {t.border}"></div>
                      <div class="tp-content">
                        <div class="tp-card" style="background: {t.surface}; border: 1px solid {t.border}"></div>
                        <div class="tp-card" style="background: {t.surface}; border: 1px solid {t.border}; opacity:0.7"></div>
                      </div>
                    </div>
                  </div>
                </div>
                <span class="theme-label">{t.label}</span>
                {#if uiTheme === t.id}
                  <div class="theme-check" style="background: {accentHex}">
                    <svg viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2.5" width="10" height="10"><polyline points="20 6 9 17 4 12"/></svg>
                  </div>
                {/if}
              </button>
            {/each}
          </div>
        </div>

        <!-- Border Radius -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.radius')}</div>
          <div class="radius-grid">
            {#each radii as r}
              <button class="radius-card" class:selected={uiRadius === r.id} on:click={() => (uiRadius = r.id)}>
                <div class="radius-preview" style="border-radius: {r.preview}"></div>
                <span class="radius-label">{$t(r.labelKey)}</span>
              </button>
            {/each}
          </div>
        </div>

        <!-- Schriftart -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.font')}</div>
          <div class="font-grid">
            {#each fonts as f}
              <button
                class="font-card"
                class:selected={uiFont === f.id}
                on:click={() => (uiFont = f.id)}
                style="font-family: {f.stack}"
              >
                <span class="font-preview">Ag</span>
                <span class="font-name">{$t(f.labelKey)}</span>
                {#if uiFont === f.id}
                  <div class="font-check" style="background: var(--accent)">
                    <svg viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2.5" width="9" height="9"><polyline points="20 6 9 17 4 12"/></svg>
                  </div>
                {/if}
              </button>
            {/each}
          </div>
        </div>

        <!-- UI-Größe -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.uiScale')}</div>
          <div class="size-grid">
            {#each uiScales as s}
              <button class="size-card" class:selected={uiScale === s.value} on:click={() => (uiScale = s.value)}>
                <span class="size-preview" style="font-size:{10 + s.value * 8}px">Aa</span>
                <span class="size-label">{s.label}</span>
                <span class="size-px">{Math.round(s.value * 100)}%</span>
              </button>
            {/each}
          </div>
        </div>

        <!-- Layout & Schrift -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.layout')}</div>
          <div class="compact-row">
            <span class="compact-label text-muted">{$t('settings.density')}</span>
            <div class="chip-group">
              {#each densities as d}
                <button class="chip" class:active={uiDensity === d.id} on:click={() => (uiDensity = d.id)} title={$t(d.descKey)}>{$t(d.labelKey)}</button>
              {/each}
            </div>
          </div>
          <div class="compact-row">
            <span class="compact-label text-muted">{$t('settings.sidebar')}</span>
            <div class="chip-group">
              {#each sidebarWidths as sw}
                <button class="chip" class:active={uiSidebarWidth === sw.value} on:click={() => (uiSidebarWidth = sw.value)}>{$t(sw.labelKey)} <span style="opacity:.6;font-size:10px">{sw.value}px</span></button>
              {/each}
            </div>
          </div>
          <div class="compact-row">
            <span class="compact-label text-muted">{$t('settings.animations')}</span>
            <div class="toggle-switch" class:on={uiAnimations}
              on:click={() => (uiAnimations = !uiAnimations)}
              role="switch" aria-checked={uiAnimations} tabindex="0"
              on:keydown={e => e.key === ' ' && (uiAnimations = !uiAnimations)}>
              <div class="toggle-thumb"></div>
            </div>
          </div>
        </div>

      <!-- ── Instanz-Standards ── -->
      {:else if activeTab === 'instances'}

        <!-- RAM -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.ram')}</div>
          <p class="hint-text text-muted">{$t('settings.ramHint')}</p>
          <div class="setting-row">
            <div class="setting-label">
              <span>{$t('settings.ramAlloc')}</span>
              <span class="setting-value">{formatRam(globalRamMax)}</span>
            </div>
            <div class="ram-slider-wrap">
              <input type="range" min="0" max={ramSnaps.length - 1} step="1"
                value={ramMaxIdx} on:input={onRamMaxSlider} class="range-input" />
              <div class="ram-ticks">
                {#each ramSnaps as _, i}
                  <span class="ram-tick" style="left:{(i/(ramSnaps.length-1))*100}%"></span>
                {/each}
              </div>
            </div>
            <div class="ram-marks">
              {#each ramSnaps as v, i}
                {#if i % Math.ceil(ramSnaps.length / 5) === 0 || i === ramSnaps.length - 1}
                  <span
                    class="ram-mark"
                    class:ram-mark--first={i === 0}
                    class:ram-mark--last={i === ramSnaps.length - 1}
                    style="left:{(i/(ramSnaps.length-1))*100}%"
                  >{formatRam(v)}</span>
                {/if}
              {/each}
            </div>
          </div>
        </div>

        <!-- Java -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.java')}</div>
          <div class="field-row">
            <label class="field-label" for="global-java">{$t('settings.javaPath')}</label>
            <input id="global-java" class="input" type="text" placeholder={$t('settings.javaPathPlaceholder')} bind:value={globalJavaPath} />
          </div>
          <p class="hint-text text-muted">{$t('settings.javaPathHint')}</p>
        </div>

        <!-- Fenster -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.defaultWindow')}</div>
          <!-- svelte-ignore a11y-label-has-associated-control -->
          <label class="toggle-row">
            <div class="toggle-text">
              <span class="toggle-title">{$t('settings.fullscreen')}</span>
              <span class="toggle-desc text-muted">{$t('settings.fullscreenDesc')}</span>
            </div>
            <div class="toggle-switch" class:on={defaultFullscreen}
              on:click={() => (defaultFullscreen = !defaultFullscreen)}
              role="switch" aria-checked={defaultFullscreen} tabindex="0"
              on:keydown={e => e.key === ' ' && (defaultFullscreen = !defaultFullscreen)}>
              <div class="toggle-thumb"></div>
            </div>
          </label>
          {#if !defaultFullscreen}
            <div class="res-row">
              <div class="res-field">
                <label class="field-label" for="def-w">{$t('settings.width')}</label>
                <input id="def-w" class="input" type="number" min="640" max="7680" bind:value={defaultWidth} />
              </div>
              <span class="res-sep text-muted">×</span>
              <div class="res-field">
                <label class="field-label" for="def-h">{$t('settings.height')}</label>
                <input id="def-h" class="input" type="number" min="480" max="4320" bind:value={defaultHeight} />
              </div>
            </div>
            <div class="preset-row">
              {#each windowPresets as p}
                <button class="chip" class:active={defaultWidth === p.w && defaultHeight === p.h}
                  on:click={() => { defaultWidth = p.w; defaultHeight = p.h }}>{p.label}</button>
              {/each}
            </div>
          {/if}
        </div>

        <!-- JVM-Argumente -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.jvmArgs')}</div>
          <textarea
            class="input textarea-mono"
            rows="3"
            placeholder="-XX:+UseG1GC -XX:MaxGCPauseMillis=50"
            bind:value={defaultCustomJvmArgs}
          ></textarea>
          <p class="hint-text text-muted">{$t('settings.jvmArgsHint')}</p>
        </div>

        <!-- Umgebungsvariablen -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.envVars')}</div>
          <div class="env-list">
            {#each defaultEnvVars as _row, i}
              <div class="env-row">
                <input class="input env-key" type="text" placeholder="VARIABLE" bind:value={defaultEnvVars[i][0]} />
                <span class="env-eq text-muted">=</span>
                <input class="input env-val" type="text" placeholder={$t('settings.envVarValue')} bind:value={defaultEnvVars[i][1]} />
                <button class="btn-icon-sm" on:click={() => removeEnvVar(i)} title={$t('common.remove')}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                </button>
              </div>
            {/each}
          </div>
          <button class="btn btn-ghost btn-sm" style="width:fit-content" on:click={addEnvVar}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
            {$t('settings.addEnvVar')}
          </button>
        </div>

        <!-- Launch Hooks -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.hooks')}</div>
          <div class="hook-field">
            <label class="field-label" for="pre">{$t('settings.preLaunch')}</label>
            <input id="pre" class="input input-mono" type="text" placeholder={$t('settings.preLaunchHint')} bind:value={defaultPreLaunchHook} />
          </div>
          <div class="hook-field">
            <label class="field-label" for="wrap">{$t('settings.wrapper')}</label>
            <input id="wrap" class="input input-mono" type="text" placeholder={$t('settings.wrapperHint')} bind:value={defaultWrapperCommand} />
          </div>
          <div class="hook-field">
            <label class="field-label" for="post">{$t('settings.postExit')}</label>
            <input id="post" class="input input-mono" type="text" placeholder={$t('settings.postExitHint')} bind:value={defaultPostExitHook} />
          </div>
        </div>

      <!-- ── Verbesserungen ── -->
      {:else if activeTab === 'improvements'}

        <!-- Screenshots -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.impScreenshotsTitle')}</div>
          <!-- svelte-ignore a11y-label-has-associated-control -->
          <label class="toggle-row">
            <div class="toggle-text">
              <span class="toggle-title">{$t('settings.impAutoCompress')}</span>
              <span class="toggle-desc text-muted">{$t('settings.impAutoCompressDesc')}</span>
            </div>
            <div class="toggle-switch" class:on={autoCompressScreenshots}
              on:click={() => (autoCompressScreenshots = !autoCompressScreenshots)}
              role="switch" aria-checked={autoCompressScreenshots} tabindex="0"
              on:keydown={e => e.key === ' ' && (autoCompressScreenshots = !autoCompressScreenshots)}>
              <div class="toggle-thumb"></div>
            </div>
          </label>
          <!-- svelte-ignore a11y-label-has-associated-control -->
          <label class="toggle-row">
            <div class="toggle-text">
              <span class="toggle-title">{$t('settings.impScreenshotSync')}</span>
              <span class="toggle-desc text-muted">{$t('settings.impScreenshotSyncDesc')}</span>
            </div>
            <div class="toggle-switch" class:on={screenshotSync}
              on:click={() => (screenshotSync = !screenshotSync)}
              role="switch" aria-checked={screenshotSync} tabindex="0"
              on:keydown={e => e.key === ' ' && (screenshotSync = !screenshotSync)}>
              <div class="toggle-thumb"></div>
            </div>
          </label>
        </div>

        <!-- Welten & Server -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.impWorldsTitle')}</div>
          <!-- svelte-ignore a11y-label-has-associated-control -->
          <label class="toggle-row">
            <div class="toggle-text">
              <span class="toggle-title">{$t('settings.impSavesSync')}</span>
              <span class="toggle-desc text-muted">{$t('settings.impSavesSyncDesc')}</span>
            </div>
            <div class="toggle-switch" class:on={savesSync}
              on:click={() => (savesSync = !savesSync)}
              role="switch" aria-checked={savesSync} tabindex="0"
              on:keydown={e => e.key === ' ' && (savesSync = !savesSync)}>
              <div class="toggle-thumb"></div>
            </div>
          </label>
          <!-- svelte-ignore a11y-label-has-associated-control -->
          <label class="toggle-row">
            <div class="toggle-text">
              <span class="toggle-title">{$t('settings.impServersSync')}</span>
              <span class="toggle-desc text-muted">{$t('settings.impServersSyncDesc')}</span>
            </div>
            <div class="toggle-switch" class:on={serversSync}
              on:click={() => (serversSync = !serversSync)}
              role="switch" aria-checked={serversSync} tabindex="0"
              on:keydown={e => e.key === ' ' && (serversSync = !serversSync)}>
              <div class="toggle-thumb"></div>
            </div>
          </label>
        </div>

        <!-- Konfiguration -->
        <div class="card settings-card">
          <div class="card-section-title">{$t('settings.impConfigTitle')}</div>
          <!-- svelte-ignore a11y-label-has-associated-control -->
          <label class="toggle-row">
            <div class="toggle-text">
              <span class="toggle-title">{$t('settings.impConfigSync')}</span>
              <span class="toggle-desc text-muted">{$t('settings.impConfigSyncDesc')}</span>
            </div>
            <div class="toggle-switch" class:on={configSync}
              on:click={() => (configSync = !configSync)}
              role="switch" aria-checked={configSync} tabindex="0"
              on:keydown={e => e.key === ' ' && (configSync = !configSync)}>
              <div class="toggle-thumb"></div>
            </div>
          </label>
          <!-- svelte-ignore a11y-label-has-associated-control -->
          <label class="toggle-row">
            <div class="toggle-text">
              <span class="toggle-title">{$t('settings.impResourcePackSync')}</span>
              <span class="toggle-desc text-muted">{$t('settings.impResourcePackSyncDesc')}</span>
            </div>
            <div class="toggle-switch" class:on={resourcePackSync}
              on:click={() => (resourcePackSync = !resourcePackSync)}
              role="switch" aria-checked={resourcePackSync} tabindex="0"
              on:keydown={e => e.key === ' ' && (resourcePackSync = !resourcePackSync)}>
              <div class="toggle-thumb"></div>
            </div>
          </label>
        </div>


      {:else if activeTab === 'java'}
        <div class="card settings-card">
          <div class="card-title">{$t('settings.tabJava')}</div>
          <p class="hint-text text-muted">{$t('settings.javaLocalHint')}</p>
        </div>

        {#each JAVA_VERSIONS as jv}
          {@const op = javaOp[jv.ver]}
          {@const st = javaStatus[jv.ver]}
          {@const busy = op !== null}
          {@const installed = !!getJavaPath(jv.ver).trim()}
          <div class="card settings-card java-card">
            <div class="java-card-header">
              <span class="java-card-title">{jv.label}</span>
              <span class="java-card-mc text-muted">{jv.mc}</span>
            </div>
            <div class="java-path-row">
              {#if jv.ver === '8'}
                <input class="input java-path-input" type="text" placeholder="/usr/lib/jvm/java-8/bin/java" bind:value={javaPath8} />
              {:else if jv.ver === '17'}
                <input class="input java-path-input" type="text" placeholder="/usr/lib/jvm/java-17/bin/java" bind:value={javaPath17} />
              {:else if jv.ver === '21'}
                <input class="input java-path-input" type="text" placeholder="/usr/lib/jvm/java-21/bin/java" bind:value={javaPath21} />
              {:else if jv.ver === '25'}
                <input class="input java-path-input" type="text" placeholder="/usr/lib/jvm/java-25/bin/java" bind:value={javaPath25} />
              {:else}
                <input class="input java-path-input" type="text" placeholder="/usr/lib/jvm/java-26/bin/java" bind:value={javaPath26} />
              {/if}
            </div>
            <div class="java-actions">
              <!-- Installieren / Installiert -->
              <button class="btn btn-sm" class:btn-primary={!installed} class:btn-success={installed} disabled={busy || installed} on:click={() => installJava(jv.ver)}>
                {#if op === 'install'}
                  <svg class="btn-icon spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"/></svg>
                  {$t('settings.javaInstalling')}
                {:else if installed}
                  <svg class="btn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>
                  {$t('settings.javaInstalled')}
                {:else}
                  <svg class="btn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 3v13M5 14l7 7 7-7"/><path d="M3 21h18"/></svg>
                  {$t('settings.javaInstall')}
                {/if}
              </button>
              <!-- Erkennen -->
              <button class="btn btn-ghost btn-sm" disabled={busy} on:click={() => detectJava(jv.ver)}>
                {#if op === 'detect'}
                  <svg class="btn-icon spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"/></svg>
                {:else}
                  <svg class="btn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                {/if}
                {op === 'detect' ? $t('settings.javaDetecting') : $t('settings.javaDetect')}
              </button>
              <!-- Testen -->
              <button class="btn btn-ghost btn-sm" disabled={busy || !getJavaPath(jv.ver).trim()} on:click={() => testJava(jv.ver)}>
                {#if op === 'test'}
                  <svg class="btn-icon spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"/></svg>
                {:else}
                  <svg class="btn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="5 3 19 12 5 21 5 3"/></svg>
                {/if}
                {op === 'test' ? $t('settings.javaTesting') : $t('settings.javaTest')}
              </button>
            </div>
            {#if st && !st.ok}
              <div class="java-status java-err">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
                <span class="java-status-text">{st.text}</span>
              </div>
            {/if}
          </div>
        {/each}

      {/if}
    {/if}
  </div>
</div>

{#if legalModal}
  <div class="legal-backdrop" on:click={() => (legalModal = '')} role="button" tabindex="-1" on:keydown={e => e.key === 'Escape' && (legalModal = '')}>
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
    <div class="legal-modal" on:click|stopPropagation role="dialog">
      <div class="legal-modal-header">
        <span class="legal-modal-title">{legalModal === 'impressum' ? $t('settings.impressum') : $t('settings.privacy')}</span>
        <button class="legal-close" on:click={() => (legalModal = '')}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        </button>
      </div>
      <div class="legal-modal-body">

        {#if legalModal === 'impressum'}

          <div class="legal-section">
            <p class="legal-text">{$t('settings.impressumDisclaimer')}</p>
          </div>

          <div class="legal-section">
            <div class="legal-section-title">{$t('settings.impressumRustTitle')}</div>
            <div class="lib-list">
              {#each [
                ['Tauri 2', 'MIT / Apache 2.0'],
                ['tauri-plugin-shell / -dialog', 'MIT / Apache 2.0'],
                ['Tokio', 'MIT'],
                ['reqwest', 'MIT / Apache 2.0'],
                ['serde / serde_json', 'MIT / Apache 2.0'],
                ['keyring', 'MIT'],
                ['uuid', 'MIT / Apache 2.0'],
                ['dirs', 'MIT / Apache 2.0'],
                ['chrono', 'MIT / Apache 2.0'],
                ['anyhow / thiserror', 'MIT / Apache 2.0'],
                ['sha1 / hex / base64', 'MIT / Apache 2.0'],
                ['zip / flate2', 'MIT / Apache 2.0'],
                ['url / urlencoding', 'MIT / Apache 2.0'],
                ['rand', 'MIT / Apache 2.0'],
                ['open', 'MIT'],
                ['tracing / tracing-subscriber', 'MIT'],
                ['futures / futures-util', 'MIT / Apache 2.0'],
              ] as [name, lic]}
                <div class="lib-row"><span class="lib-name">{name}</span><span class="lib-license text-muted">{lic}</span></div>
              {/each}
            </div>
          </div>

          <div class="legal-section">
            <div class="legal-section-title">{$t('settings.impressumJsTitle')}</div>
            <div class="lib-list">
              {#each [
                ['Svelte 4', 'MIT'],
                ['Vite 5', 'MIT'],
                ['@tauri-apps/api 2', 'MIT / Apache 2.0'],
                ['@tauri-apps/plugin-shell 2', 'MIT / Apache 2.0'],
                ['three.js 0.170', 'MIT'],
                ['skinview3d 2.2', 'MIT'],
              ] as [name, lic]}
                <div class="lib-row"><span class="lib-name">{name}</span><span class="lib-license text-muted">{lic}</span></div>
              {/each}
            </div>
          </div>

          <div class="legal-section">
            <div class="legal-section-title">{$t('settings.impressumFontsTitle')}</div>
            <div class="lib-list">
              {#each [
                ['Minecraft (South-Paw/typeface-minecraft)', $t('settings.impressumFontFreeware')],
                ['Nunito', 'SIL OFL 1.1 · Google Fonts'],
                ['Oxanium', 'SIL OFL 1.1 · Google Fonts'],
                ['Merriweather', 'SIL OFL 1.1 · Google Fonts'],
                ['JetBrains Mono', 'SIL OFL 1.1 · Google Fonts'],
              ] as [name, lic]}
                <div class="lib-row"><span class="lib-name">{name}</span><span class="lib-license text-muted">{lic}</span></div>
              {/each}
            </div>
          </div>

          <div class="legal-section">
            <div class="legal-section-title">{$t('settings.impressumServicesTitle')}</div>
            <div class="lib-list">
              {#each [
                ['Microsoft Identity Platform', $t('settings.impressumSvcOauth')],
                ['Xbox Live / XSTS', $t('settings.impressumSvcXbox')],
                ['api.minecraftservices.com', $t('settings.impressumSvcMcServices')],
                ['sessionserver.mojang.com', $t('settings.impressumSvcSession')],
                ['launchermeta.mojang.com', $t('settings.impressumSvcVersions')],
                ['resources.download.minecraft.net', $t('settings.impressumSvcResources')],
                ['libraries.minecraft.net', $t('settings.impressumSvcLibraries')],
                ['api.modrinth.com', $t('settings.impressumSvcModrinth')],
                ['catbox.moe', $t('settings.impressumSvcCatbox')],
                ['Discord IPC', $t('settings.impressumSvcDiscord')],
                ['fonts.googleapis.com', $t('settings.impressumSvcGoogleFonts')],
                ['cdn.jsdelivr.net', $t('settings.impressumSvcJsdelivr')],
              ] as [name, note]}
                <div class="lib-row"><span class="lib-name">{name}</span><span class="lib-license text-muted">{note}</span></div>
              {/each}
            </div>
          </div>

        {:else}

          <div class="legal-section">
            <p class="legal-text">{$t('settings.privacyIntro')}</p>
          </div>

          <div class="legal-section">
            <div class="legal-section-title">{$t('settings.privacyMicrosoftTitle')}</div>
            <p class="legal-text">{$t('settings.privacyMicrosoftText')}</p>
            <p class="legal-text"><span class="legal-link">https://privacy.microsoft.com/privacystatement</span></p>
          </div>

          <div class="legal-section">
            <div class="legal-section-title">{$t('settings.privacyMojangTitle')}</div>
            <ul class="legal-list">
              <li><strong>api.minecraftservices.com</strong> — {$t('settings.privacyMojangItem1')}</li>
              <li><strong>sessionserver.mojang.com</strong> — {$t('settings.privacyMojangItem2')}</li>
              <li><strong>launchermeta.mojang.com / resources / libraries</strong> — {$t('settings.privacyMojangItem3')}</li>
            </ul>
            <p class="legal-text"><span class="legal-link">https://www.minecraft.net/privacy</span></p>
          </div>

          <div class="legal-section">
            <div class="legal-section-title">{$t('settings.privacyCatboxTitle')}</div>
            <p class="legal-text">{$t('settings.privacyCatboxText')}</p>
            <p class="legal-text" style="color:var(--warning)">⚠ {$t('settings.privacyCatboxWarning')}</p>
            <p class="legal-text"><span class="legal-link">https://catbox.moe/tools.php</span></p>
          </div>

          <div class="legal-section">
            <div class="legal-section-title">Modrinth</div>
            <p class="legal-text">{$t('settings.privacyModrinthText')}</p>
            <p class="legal-text"><span class="legal-link">https://modrinth.com/legal/privacy</span></p>
          </div>

          <div class="legal-section">
            <div class="legal-section-title">Discord Rich Presence</div>
            <p class="legal-text">{$t('settings.privacyDiscordText')}</p>
            <p class="legal-text"><span class="legal-link">https://discord.com/privacy</span></p>
          </div>

          <div class="legal-section">
            <div class="legal-section-title">Google Fonts &amp; jsDelivr</div>
            <p class="legal-text">{$t('settings.privacyGoogleText')}</p>
            <p class="legal-text"><span class="legal-link">https://policies.google.com/privacy</span> · <span class="legal-link">https://www.jsdelivr.com/privacy-policy</span></p>
          </div>

          <div class="legal-section">
            <div class="legal-section-title">{$t('settings.privacyStorageTitle')}</div>
            <ul class="legal-list">
              <li><code class="legal-code">~/.config/nova-launcher/config.json</code> — {$t('settings.privacyStorageItem1')}</li>
              <li>{$t('settings.privacyStorageItem2')}</li>
              <li><code class="legal-code">~/.local/share/nova-launcher/</code> — {$t('settings.privacyStorageItem3')}</li>
            </ul>
          </div>

        {/if}

      </div>
    </div>
  </div>
{/if}

<style>
  .tabs {
    display: flex; gap: 2px; padding: 0 24px;
    border-bottom: 1px solid var(--border); flex-shrink: 0;
  }
  .tab {
    padding: 11px 16px; font-size: 13px; color: var(--text-muted);
    border-bottom: 2px solid transparent; margin-bottom: -1px;
    transition: all var(--transition); background: none;
    border-left: none; border-right: none; border-top: none;
    font-weight: 500;
  }
  .tab:hover { color: var(--text); }
  .tab.active { color: var(--accent); border-bottom-color: var(--accent); font-weight: 600; }

  .save-indicator { display: flex; align-items: center; height: 28px; }
  .status-saving, .status-saved, .status-error {
    display: flex; align-items: center; gap: 6px; font-size: 12px;
    border-radius: 100px; padding: 4px 10px;
  }
  .status-saving { color: var(--text-muted); background: var(--surface2); }
  .status-saved { color: var(--success); background: rgba(52,211,153,0.1); }
  .status-error { color: var(--error); background: rgba(248,113,113,0.1); cursor: help; }

  .dot-spin {
    width: 12px; height: 12px; border-radius: 50%;
    border: 2px solid var(--border); border-top-color: var(--accent);
    animation: spin 0.7s linear infinite; flex-shrink: 0;
  }
  @keyframes spin { to { transform: rotate(360deg) } }
  .loading-cfg { display: flex; align-items: center; gap: 10px; padding: 40px 0; justify-content: center; }

  /* Cards */
  .settings-card { padding: 20px; margin-bottom: 14px; display: flex; flex-direction: column; gap: 14px; box-shadow: 0 1px 4px rgba(0,0,0,0.12); }
  .card-section-title { font-size: 11px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.08em; color: var(--text-muted); padding-left: 9px; border-left: 2px solid var(--accent); }
  .hint-text { font-size: 12px; line-height: 1.5; margin-top: -6px; }

  /* Language chips */
  .lang-row { display: flex; gap: 8px; }
  .lang-chip {
    padding: 7px 20px; border-radius: var(--radius-sm);
    border: 1px solid var(--border); background: var(--surface2);
    font-size: 13px; color: var(--text-dim); cursor: pointer;
    transition: all 0.12s ease;
  }
  .lang-chip:hover { border-color: var(--text-muted); color: var(--text); }
  .lang-chip.active { border-color: var(--accent); color: var(--accent); background: color-mix(in srgb, var(--accent) 8%, var(--surface2)); font-weight: 500; }

  /* Discord RPC */
  .rpc-card { gap: 16px; }
  .rpc-header { display: flex; align-items: center; gap: 12px; }
  .rpc-icon-wrap {
    width: 36px; height: 36px; border-radius: var(--radius-sm);
    background: #5865F2; color: #fff;
    display: flex; align-items: center; justify-content: center; flex-shrink: 0;
  }
  .rpc-title-block { display: flex; flex-direction: column; gap: 2px; flex: 1; }


  /* App folder */
  .folder-row { display: flex; align-items: center; gap: 12px; }
  .folder-path {
    flex: 1; font-size: 12px; font-family: monospace;
    background: var(--surface2); border: 1px solid var(--border);
    border-radius: var(--radius-sm); padding: 7px 10px;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }

  /* Cache */
  .cache-row { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
  .cache-info { display: flex; flex-direction: column; gap: 2px; }

  /* Toggle */
  .toggle-list { display: flex; flex-direction: column; }
  .toggle-row {
    display: flex; align-items: center; justify-content: space-between;
    gap: 16px; padding: 12px 0; cursor: pointer;
    border-bottom: 1px solid var(--border);
  }
  .toggle-row:last-child { border-bottom: none; padding-bottom: 0; }
  .toggle-text { display: flex; flex-direction: column; gap: 2px; }
  .toggle-title { font-size: 13px; color: var(--text-dim); }
  .toggle-desc { font-size: 11px; }
  .toggle-switch {
    width: 42px; height: 24px; border-radius: 100px;
    background: var(--surface3); border: 1px solid var(--border);
    position: relative; flex-shrink: 0;
    transition: all 0.2s ease; cursor: pointer;
  }
  .toggle-switch.on { background: var(--accent); border-color: var(--accent); box-shadow: 0 0 8px rgba(var(--accent-rgb),0.4); }
  .toggle-thumb {
    width: 18px; height: 18px; border-radius: 50%; background: var(--text-muted);
    position: absolute; top: 2px; left: 2px; transition: all 0.2s ease;
    box-shadow: 0 1px 4px rgba(0,0,0,0.3);
  }
  .toggle-switch.on .toggle-thumb { left: 20px; background: white; }

  /* Fields */
  .field-row { display: flex; align-items: center; gap: 12px; }
  .field-label { font-size: 12px; color: var(--text-dim); white-space: nowrap; min-width: 70px; }

  /* Sliders */
  .setting-row { display: flex; flex-direction: column; gap: 8px; }
  .setting-label { display: flex; justify-content: space-between; font-size: 13px; color: var(--text-dim); }
  .setting-value { font-weight: 600; color: var(--accent); font-size: 13px; }
  .range-input { width: 100%; accent-color: var(--accent); cursor: pointer; height: 4px; }
  .ram-slider-wrap { position: relative; }
  .ram-ticks { position: relative; height: 5px; margin-top: 1px; }
  .ram-tick { position: absolute; top: 0; width: 1.5px; height: 5px; background: var(--border); transform: translateX(-50%); border-radius: 1px; }
  .ram-marks { position: relative; height: 16px; margin-top: 2px; }
  .ram-mark { position: absolute; transform: translateX(-50%); font-size: 10px; color: var(--text-muted); top: 0; white-space: nowrap; }
  .ram-mark--first { transform: translateX(0); }
  .ram-mark--last  { transform: translateX(-100%); }

  /* Resolution */
  .res-row { display: flex; align-items: center; gap: 12px; }
  .res-field { display: flex; align-items: center; gap: 10px; flex: 1; }
  .res-sep { font-size: 18px; font-weight: 300; flex-shrink: 0; }
  .preset-row { display: flex; gap: 6px; }

  /* Env vars */
  .env-list { display: flex; flex-direction: column; gap: 6px; }
  .env-row { display: flex; align-items: center; gap: 6px; }
  .env-key { width: 140px; flex-shrink: 0; font-family: monospace; font-size: 12px; }
  .env-val { flex: 1; font-family: monospace; font-size: 12px; }
  .env-eq { font-size: 14px; font-weight: 600; flex-shrink: 0; }
  .btn-icon-sm {
    width: 26px; height: 26px; flex-shrink: 0;
    display: flex; align-items: center; justify-content: center;
    border-radius: var(--radius-sm); border: 1px solid var(--border);
    background: transparent; color: var(--text-muted); cursor: pointer;
    transition: all 0.12s ease;
  }
  .btn-icon-sm:hover { border-color: var(--error); color: var(--error); background: rgba(248,113,113,0.08); }

  /* Hook fields */
  .hook-field { display: flex; flex-direction: column; gap: 5px; }
  .input-mono { font-family: monospace; font-size: 12px; }
  .textarea-mono { font-family: monospace; font-size: 12px; resize: vertical; min-height: 60px; }

  /* Color picker */
  .color-picker-row { display: flex; align-items: flex-start; gap: 16px; }
  .color-picker-wrap { position: relative; width: 60px; height: 60px; flex-shrink: 0; cursor: pointer; border-radius: var(--radius); overflow: hidden; box-shadow: 0 4px 16px rgba(0,0,0,0.35); }
  .native-color-input { position: absolute; inset: 0; width: 100%; height: 100%; opacity: 0; cursor: pointer; border: none; padding: 0; }
  .color-swatch-big { width: 60px; height: 60px; display: flex; align-items: center; justify-content: center; pointer-events: none; }
  .color-info { display: flex; flex-direction: column; gap: 6px; flex: 1; }
  .color-hex-label { font-size: 20px; font-weight: 700; font-family: monospace; color: var(--text); letter-spacing: 0.05em; }
  .accent-preview { display: flex; align-items: center; gap: 8px; margin-top: 4px; }
  .ap-btn { padding: 3px 10px; border-radius: var(--radius-sm); font-size: 11px; font-weight: 500; border: 1px solid transparent; color: white; }
  .ap-tag { padding: 2px 8px; border-radius: 100px; font-size: 11px; font-weight: 500; border: 1px solid; }
  .ap-dot { width: 10px; height: 10px; border-radius: 50%; }
  .color-presets-grid { display: grid; grid-template-columns: repeat(16, 1fr); gap: 5px; }
  .preset-swatch { width: 20px; height: 20px; border-radius: 50%; border: 2px solid transparent; cursor: pointer; transition: all 0.12s ease; flex-shrink: 0; }
  .preset-swatch:hover { transform: scale(1.25); }
  .preset-swatch.active { border-color: white; box-shadow: 0 0 0 2px var(--ring, var(--accent)); transform: scale(1.2); }

  /* Theme picker */
  .theme-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 10px; }
  .theme-card { display: flex; flex-direction: column; align-items: center; gap: 7px; padding: 10px 8px; border-radius: var(--radius); border: 1px solid var(--border); background: var(--surface2); cursor: pointer; transition: all 0.15s ease; position: relative; box-shadow: 0 1px 3px rgba(0,0,0,0.1); }
  .theme-card:hover { border-color: rgba(var(--accent-rgb),0.4); transform: translateY(-1px); box-shadow: 0 4px 12px rgba(0,0,0,0.2); }
  .theme-card.selected { border-color: var(--accent); box-shadow: 0 0 0 1px rgba(var(--accent-rgb),0.3), 0 4px 16px rgba(0,0,0,0.2); background: color-mix(in srgb, var(--accent) 6%, var(--surface2)); }
  .theme-preview { width: 100%; height: 52px; border-radius: 4px; overflow: hidden; }
  .tp-bg { width: 100%; height: 100%; display: flex; }
  .tp-sidebar { width: 30%; height: 100%; padding: 4px 3px; display: flex; flex-direction: column; gap: 2px; }
  .tp-nav-item { height: 6px; border-radius: 1px; }
  .tp-nav-item2 { height: 4px; border-radius: 1px; }
  .tp-main { flex: 1; display: flex; flex-direction: column; }
  .tp-bar { height: 10px; }
  .tp-content { flex: 1; padding: 3px; display: flex; flex-direction: column; gap: 2px; }
  .tp-card { flex: 1; border-radius: 2px; }
  .theme-label { font-size: 11px; font-weight: 500; color: var(--text-dim); }
  .theme-check { position: absolute; top: 5px; right: 5px; width: 15px; height: 15px; border-radius: 50%; display: flex; align-items: center; justify-content: center; }

  /* Radius picker */
  .radius-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; }
  .radius-card { display: flex; flex-direction: column; align-items: center; gap: 10px; padding: 14px 10px; border-radius: var(--radius); border: 1px solid var(--border); background: var(--surface2); cursor: pointer; transition: all 0.15s ease; box-shadow: 0 1px 3px rgba(0,0,0,0.1); }
  .radius-card:hover { border-color: rgba(var(--accent-rgb),0.4); transform: translateY(-1px); box-shadow: 0 4px 12px rgba(0,0,0,0.2); }
  .radius-card.selected { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 6%, var(--surface2)); box-shadow: 0 0 0 1px rgba(var(--accent-rgb),0.3); }
  .radius-preview { width: 36px; height: 36px; background: var(--surface3); border: 1px solid var(--border); }
  .radius-label { font-size: 11px; font-weight: 500; color: var(--text-dim); }

  /* Chip rows */
  .compact-row { display: flex; align-items: center; gap: 12px; }
  .compact-label { font-size: 12px; white-space: nowrap; min-width: 80px; }
  .chip-group { display: flex; gap: 5px; flex-wrap: wrap; }
  .chip { display: inline-flex; align-items: center; gap: 4px; padding: 4px 10px; border-radius: var(--radius-sm); border: 1px solid var(--border); background: var(--surface2); font-size: 12px; color: var(--text-dim); cursor: pointer; transition: all 0.12s ease; white-space: nowrap; }
  .chip:hover { border-color: var(--text-muted); color: var(--text); }
  .chip.active { border-color: var(--accent); color: var(--accent); background: color-mix(in srgb, var(--accent) 8%, var(--surface2)); }

  /* Font picker */
  .font-grid { display: grid; grid-template-columns: repeat(5, 1fr); gap: 8px; }
  .font-card { display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 14px 8px 10px; border-radius: var(--radius); border: 1px solid var(--border); background: var(--surface2); cursor: pointer; transition: all 0.15s ease; position: relative; box-shadow: 0 1px 3px rgba(0,0,0,0.1); }
  .font-card:hover { border-color: rgba(var(--accent-rgb),0.4); transform: translateY(-1px); box-shadow: 0 4px 12px rgba(0,0,0,0.2); }
  .font-card.selected { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 6%, var(--surface2)); box-shadow: 0 0 0 1px rgba(var(--accent-rgb),0.3); }
  .font-preview { font-size: 26px; font-weight: 700; color: var(--text); line-height: 1; }
  .font-name { font-size: 11px; color: var(--text-dim); font-family: inherit; }
  .font-card.selected .font-name { color: var(--accent); }
  .font-check { position: absolute; top: 5px; right: 5px; width: 14px; height: 14px; border-radius: 50%; display: flex; align-items: center; justify-content: center; }

  /* UI-Größe picker */
  .size-grid { display: grid; grid-template-columns: repeat(5, 1fr); gap: 8px; }
  .size-card { display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 14px 8px 10px; border-radius: var(--radius); border: 1px solid var(--border); background: var(--surface2); cursor: pointer; transition: all 0.15s ease; }
  .size-card:hover { border-color: var(--text-muted); }
  .size-card.selected { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 6%, var(--surface2)); }
  .size-preview { font-weight: 700; color: var(--text); line-height: 1; }
  .size-card.selected .size-preview { color: var(--accent); }
  .size-label { font-size: 11px; font-weight: 600; color: var(--text-dim); }
  .size-px { font-size: 10px; color: var(--text-muted); }

  /* App footer */
.app-footer { display: flex; align-items: center; gap: 7px; padding: 10px 0 4px; color: var(--text-muted); font-size: 11.5px; }
  .footer-sep { color: var(--border); }
  .footer-link { background: none; border: none; color: var(--text-muted); font-size: 11.5px; cursor: pointer; padding: 0; transition: color var(--transition); }
  .footer-link:hover { color: var(--accent); }

  /* Legal modal */
  .legal-backdrop { position: fixed; inset: 0; background: rgba(0,0,0,0.55); backdrop-filter: blur(4px); z-index: 200; display: flex; align-items: center; justify-content: center; }
  .legal-modal { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); width: 560px; max-width: 90vw; max-height: 80vh; display: flex; flex-direction: column; box-shadow: 0 24px 60px rgba(0,0,0,0.5); }
  .legal-modal-header { display: flex; align-items: center; justify-content: space-between; padding: 16px 20px; border-bottom: 1px solid var(--border); flex-shrink: 0; }
  .legal-modal-title { font-size: 14px; font-weight: 700; color: var(--text); }
  .legal-close { background: none; border: none; color: var(--text-muted); cursor: pointer; padding: 4px; border-radius: var(--radius-sm); transition: all var(--transition); display: flex; }
  .legal-close:hover { color: var(--text); background: var(--surface2); }
  .legal-modal-body { overflow-y: auto; padding: 20px; display: flex; flex-direction: column; gap: 20px; }
  .legal-section { display: flex; flex-direction: column; gap: 6px; }
  .legal-section-title { font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.08em; color: var(--text-muted); margin-bottom: 2px; }
  .legal-text { font-size: 12px; line-height: 1.6; color: var(--text-dim); }
  .legal-list { list-style: none; display: flex; flex-direction: column; gap: 4px; }
  .legal-list li { font-size: 12px; color: var(--text-dim); line-height: 1.5; padding-left: 12px; position: relative; }
  .legal-list li::before { content: '·'; position: absolute; left: 0; color: var(--accent); }
  .legal-list li strong { color: var(--text); font-weight: 600; }
  .legal-link { font-size: 11px; color: var(--accent); font-family: 'JetBrains Mono', monospace; word-break: break-all; }
  .legal-code { font-family: 'JetBrains Mono', monospace; font-size: 11px; background: var(--surface2); padding: 1px 5px; border-radius: 3px; color: var(--text-dim); }
  .lib-list { display: flex; flex-direction: column; }
  .lib-row { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; padding: 4px 0; border-bottom: 1px solid var(--border); }
  .lib-row:last-child { border-bottom: none; }
  .lib-name { font-size: 12px; font-weight: 500; color: var(--text); flex-shrink: 0; }
  .lib-license { font-size: 11px; text-align: right; }

  /* Java button extras */
  .btn-success { background: rgba(34,197,94,0.15); color: #4ade80; border: 1px solid rgba(34,197,94,0.3); }
  .btn-success:disabled { opacity: 1; cursor: default; }
  .btn-icon { width: 14px; height: 14px; flex-shrink: 0; vertical-align: middle; margin-right: 4px; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .spin { animation: spin 0.8s linear infinite; display: inline-block; }

  /* Java version management */
  .java-card { display: flex; flex-direction: column; gap: 10px; }
  .java-card-header { display: flex; align-items: baseline; gap: 10px; }
  .java-card-title { font-size: 15px; font-weight: 600; color: var(--text); }
  .java-card-mc { font-size: 12px; }
  .java-path-row { display: flex; gap: 8px; align-items: center; }
  .java-path-input { flex: 1; min-width: 0; font-family: 'JetBrains Mono', monospace; font-size: 12px; }
  .java-actions { display: flex; gap: 8px; flex-wrap: wrap; }
  .java-status { display: flex; align-items: flex-start; gap: 6px; font-size: 12px; font-family: 'JetBrains Mono', monospace; padding: 6px 10px; border-radius: var(--radius); }
  .java-err { background: rgba(239,68,68,0.12); color: #f87171; }
  .java-status-text { word-break: break-all; }
</style>
