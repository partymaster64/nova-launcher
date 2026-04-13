<script>
  import { onMount, createEventDispatcher } from 'svelte'
  import { invoke, Channel } from '@tauri-apps/api/core'
  import { config, accounts } from '../../store.js'

  const dispatch = createEventDispatcher()

  export let isRestart = false

  // ── State ────────────────────────────────────────────────────────────────────
  let currentStep = 0          // 0=welcome, 1=language, 2=design, 3=discord, 4=login, 5=done
  let transitioning = false

  // Step 0 — welcome animation state (skip for restart: show immediately, no transitions)
  let showLogo = isRestart
  let showTitle = isRestart
  let showSubtitle = isRestart
  let showStartBtn = isRestart

  // Step 1 — language
  let selectedLanguage = 'de'
  const languages = [
    { code: 'de', label: 'Deutsch',   flag: '🇩🇪' },
    { code: 'en', label: 'English',   flag: '🇬🇧' },
    { code: 'fr', label: 'Français',  flag: '🇫🇷' },
    { code: 'es', label: 'Español',   flag: '🇪🇸' },
    { code: 'pl', label: 'Polski',    flag: '🇵🇱' },
  ]

  // Step 2 — design (gleiche Daten wie Settings.svelte)
  let accentHex = '#a855f7'
  let uiTheme = ''
  let uiRadius = ''
  let uiFontSize = 14
  let uiDensity = ''
  let uiAnimations = true
  let uiSidebarWidth = 220

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
    { id: '',         labelKey: 'radiusStandard', preview: '8px' },
    { id: 'compact',  labelKey: 'radiusCompact',  preview: '3px' },
    { id: 'rounded',  labelKey: 'radiusRounded',  preview: '14px' },
  ]

  function hexToRgbFloat(hex) {
    return [parseInt(hex.slice(1,3),16)/255, parseInt(hex.slice(3,5),16)/255, parseInt(hex.slice(5,7),16)/255]
  }
  function toHex(v) { return Math.round(v*255).toString(16).padStart(2,'0') }
  function lighten(hex, a) { const [r,g,b]=hexToRgbFloat(hex); const c=v=>Math.min(1,v+a); return `#${toHex(c(r))}${toHex(c(g))}${toHex(c(b))}` }
  function darken(hex, a)  { const [r,g,b]=hexToRgbFloat(hex); const c=v=>Math.max(0,v-a); return `#${toHex(c(r))}${toHex(c(g))}${toHex(c(b))}` }
  function blendColors(a, b, t) {
    const [r1,g1,b1]=hexToRgbFloat(a), [r2,g2,b2]=hexToRgbFloat(b)
    return `#${toHex(r1+(r2-r1)*t)}${toHex(g1+(g2-g1)*t)}${toHex(b1+(b2-b1)*t)}`
  }

  function applyThemePreview() {
    const root = document.documentElement
    const th = themes.find(t => t.id === uiTheme) || themes[0]
    root.style.setProperty('--accent', accentHex)
    root.style.setProperty('--accent-hover', lighten(accentHex, 0.15))
    root.style.setProperty('--accent-dim', darken(accentHex, 0.3))
    root.style.setProperty('--accent-rgb', `${parseInt(accentHex.slice(1,3),16)}, ${parseInt(accentHex.slice(3,5),16)}, ${parseInt(accentHex.slice(5,7),16)}`)
    root.style.setProperty('--bg', th.bg)
    root.style.setProperty('--surface', th.surface)
    root.style.setProperty('--surface2', th.surface2)
    root.style.setProperty('--surface3', blendColors(th.surface, th.surface2, 0.5))
    root.style.setProperty('--border', th.border)
    root.style.setProperty('--text', th.text)
    root.style.setProperty('--text-dim', th.textDim)
    root.style.setProperty('--text-muted', th.textMuted)
    const isLight = uiTheme === 'light'
    root.style.setProperty('--success', isLight ? '#059669' : '#34d399')
    root.style.setProperty('--warning', isLight ? '#d97706' : '#fbbf24')
    root.style.setProperty('--error',   isLight ? '#dc2626' : '#f87171')
    root.style.setProperty('--info',    isLight ? '#2563eb' : '#60a5fa')
    if (uiRadius === 'compact') {
      root.style.setProperty('--radius', '3px'); root.style.setProperty('--radius-sm', '2px'); root.style.setProperty('--radius-lg', '6px')
    } else if (uiRadius === 'rounded') {
      root.style.setProperty('--radius', '14px'); root.style.setProperty('--radius-sm', '8px'); root.style.setProperty('--radius-lg', '20px')
    } else {
      root.style.setProperty('--radius', '8px'); root.style.setProperty('--radius-sm', '4px'); root.style.setProperty('--radius-lg', '12px')
    }
    root.style.fontSize = `${uiFontSize}px`
    root.style.setProperty('--density', uiDensity === 'compact' ? '0.7' : uiDensity === 'comfortable' ? '1.35' : '1')
    root.style.setProperty('--transition', uiAnimations ? '0.15s ease' : '0s')
    root.style.setProperty('--sidebar-width', `${uiSidebarWidth}px`)
  }

  // Live-Preview während Step 2
  $: if (currentStep === 2) { accentHex; uiTheme; uiRadius; uiFontSize; uiDensity; uiAnimations; uiSidebarWidth; applyThemePreview() }

  $: previewTheme = themes.find(t => t.id === uiTheme) || themes[0]
  $: previewRadius = uiRadius === 'compact' ? '3px' : uiRadius === 'rounded' ? '14px' : '8px'

  // Step 3 — discord
  let discordRpc = true

  // Step 4 — login
  let loginState = 'idle'   // idle | opening | waiting | success | error
  let loginError = ''

  // Step 5 — confetti
  let confettiCanvas = null
  let confettiActive = false

  // ── i18n helpers ─────────────────────────────────────────────────────────────
  const wizardT = {
    de: {
      step1Title: 'Sprache / Language',
      step2Title: 'Design anpassen',
      step3Title: 'Discord Activity',
      step3Desc: 'Zeige anderen, was du spielst',
      step4Title: 'Anmelden',
      step5Title: 'Nova Launcher ist bereit!',
      next: 'Weiter',
      back: 'Zurück',
      skip: 'Überspringen',
      startSetup: 'Setup beginnen',
      beginPlay: 'Loslegen',
      detected: 'Erkannte Sprache',
      accentColor: 'Akzentfarbe',
      borderRadius: 'Eckenradius',
      theme: 'Theme',
      layout: 'Layout',
      density: 'Dichte',
      animations: 'Animationen',
      loginWaiting: 'Warte auf Anmeldung…',
      loginOpening: 'Browser öffnet sich…',
      loginSuccess: 'Angemeldet!',
      loginBtn: 'Mit Microsoft anmelden',
      loginSkip: 'Ohne Konto fortfahren',
      enabled: 'Aktiviert',
      disabled: 'Deaktiviert',
      welcomeSubtitle: 'Dein Minecraft Launcher',
      doneSubtitle: 'Viel Spaß beim Spielen!',
      radiusStandard: 'Standard',
      radiusCompact: 'Kompakt',
      radiusRounded: 'Rund',
      colorPicker: 'Farbe wählen',
      colorPurple: 'Lila', colorViolet: 'Violett', colorIndigo: 'Indigo', colorBlue: 'Blau',
      colorCyan: 'Cyan', colorTeal: 'Türkis', colorGreen: 'Grün', colorLime: 'Limette',
      colorYellow: 'Gelb', colorOrange: 'Orange', colorRed: 'Rot', colorPink: 'Pink',
      colorRose: 'Rose', colorWhite: 'Weiß', colorSilver: 'Silber', colorGold: 'Gold',
    },
    en: {
      step1Title: 'Sprache / Language',
      step2Title: 'Customize Design',
      step3Title: 'Discord Activity',
      step3Desc: 'Show others what you\'re playing',
      step4Title: 'Sign In',
      step5Title: 'Nova Launcher is ready!',
      next: 'Next',
      back: 'Back',
      skip: 'Skip',
      startSetup: 'Start Setup',
      beginPlay: 'Let\'s go',
      detected: 'Detected Language',
      accentColor: 'Accent Color',
      borderRadius: 'Corner Radius',
      theme: 'Theme',
      layout: 'Layout',
      density: 'Density',
      animations: 'Animations',
      loginWaiting: 'Waiting for login…',
      loginOpening: 'Opening browser…',
      loginSuccess: 'Signed in!',
      loginBtn: 'Sign in with Microsoft',
      loginSkip: 'Continue without account',
      enabled: 'Enabled',
      disabled: 'Disabled',
      welcomeSubtitle: 'Your Minecraft Launcher',
      doneSubtitle: 'Have fun playing!',
      radiusStandard: 'Standard',
      radiusCompact: 'Compact',
      radiusRounded: 'Rounded',
      colorPicker: 'Choose color',
      colorPurple: 'Purple', colorViolet: 'Violet', colorIndigo: 'Indigo', colorBlue: 'Blue',
      colorCyan: 'Cyan', colorTeal: 'Teal', colorGreen: 'Green', colorLime: 'Lime',
      colorYellow: 'Yellow', colorOrange: 'Orange', colorRed: 'Red', colorPink: 'Pink',
      colorRose: 'Rose', colorWhite: 'White', colorSilver: 'Silver', colorGold: 'Gold',
    },
    fr: {
      step1Title: 'Sprache / Language',
      step2Title: 'Personnaliser le design',
      step3Title: 'Discord Activity',
      step3Desc: 'Montrez aux autres ce que vous jouez',
      step4Title: 'Se connecter',
      step5Title: 'Nova Launcher est prêt !',
      next: 'Suivant',
      back: 'Retour',
      skip: 'Passer',
      startSetup: 'Commencer',
      beginPlay: 'Allons-y',
      detected: 'Langue détectée',
      accentColor: 'Couleur d\'accent',
      borderRadius: 'Rayon des coins',
      theme: 'Thème',
      layout: 'Disposition',
      density: 'Densité',
      animations: 'Animations',
      loginWaiting: 'En attente de connexion…',
      loginOpening: 'Ouverture du navigateur…',
      loginSuccess: 'Connecté !',
      loginBtn: 'Se connecter avec Microsoft',
      loginSkip: 'Continuer sans compte',
      enabled: 'Activé',
      disabled: 'Désactivé',
      welcomeSubtitle: 'Votre lanceur Minecraft',
      doneSubtitle: 'Amusez-vous bien !',
      radiusStandard: 'Standard',
      radiusCompact: 'Compact',
      radiusRounded: 'Arrondi',
      colorPicker: 'Choisir une couleur',
      colorPurple: 'Violet', colorViolet: 'Violet foncé', colorIndigo: 'Indigo', colorBlue: 'Bleu',
      colorCyan: 'Cyan', colorTeal: 'Sarcelle', colorGreen: 'Vert', colorLime: 'Citron vert',
      colorYellow: 'Jaune', colorOrange: 'Orange', colorRed: 'Rouge', colorPink: 'Rose',
      colorRose: 'Rose clair', colorWhite: 'Blanc', colorSilver: 'Argent', colorGold: 'Or',
    },
  }
  $: lang = wizardT[selectedLanguage] || wizardT['en']

  // ── Lifecycle ─────────────────────────────────────────────────────────────────
  onMount(async () => {
    if (isRestart) {
      const currentCfg = get(config)
      if (currentCfg?.language && languages.find(l => l.code === currentCfg.language)) {
        selectedLanguage = currentCfg.language
      }
      showLogo = true; showTitle = true; showSubtitle = true; showStartBtn = true
    } else {
      // First-time setup: detect system locale
      try {
        const locale = await invoke('get_system_locale')
        if (languages.find(l => l.code === locale)) {
          selectedLanguage = locale
        }
      } catch (_) {}
      setTimeout(() => { showLogo = true }, 100)
      setTimeout(() => { showTitle = true }, 600)
      setTimeout(() => { showSubtitle = true }, 1000)
      setTimeout(() => { showStartBtn = true }, 1600)
    }
  })

  // ── Navigation ────────────────────────────────────────────────────────────────
  async function goNext() {
    if (transitioning) return
    if (currentStep === 5) {
      await finishSetup()
      return
    }
    transitioning = true
    await tick()
    setTimeout(() => {
      currentStep += 1
      transitioning = false
      if (currentStep === 5) setTimeout(startConfetti, 0)
    }, 300)
  }

  async function goBack() {
    if (transitioning || currentStep <= 1) return
    transitioning = true
    await tick()
    setTimeout(() => {
      currentStep -= 1
      transitioning = false
    }, 300)
  }

  function startWizard() {
    transitioning = true
    setTimeout(() => {
      currentStep = 1
      transitioning = false
    }, 300)
  }

  // ── Microsoft Login ───────────────────────────────────────────────────────────
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
          loginState = 'success'
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

  // ── Confetti ──────────────────────────────────────────────────────────────────
  function startConfetti() {
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

  // ── Finish ────────────────────────────────────────────────────────────────────
  async function finishSetup() {
    try {
      await invoke('complete_setup', {
        language: selectedLanguage,
        accentColor: accentHex,
        borderRadius: uiRadius === 'compact' ? 3 : uiRadius === 'rounded' ? 14 : 8,
        theme: uiTheme || 'dark',
        discordRpc: discordRpc,
      })
      applyThemePreview()
      // Re-fetch config
      const updatedCfg = await invoke('get_config')
      config.set(updatedCfg)
    } catch (e) {
      console.error('Setup completion failed:', e)
    }
    dispatch('complete')
  }

  // ── Reactive ──────────────────────────────────────────────────────────────────


  // svelte tick
  function tick() { return new Promise(r => setTimeout(r, 0)) }

</script>

<!-- Confetti canvas -->
{#if currentStep === 5}
  <canvas bind:this={confettiCanvas}
    style="position:absolute;inset:0;width:100%;height:100%;pointer-events:none;z-index:10"
  ></canvas>
{/if}

<div class="wizard-root">

  <!-- Top bar -->
  <div class="top-bar">
    <div class="top-logo">
      <svg viewBox="0 0 100 100" width="26" height="26" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <linearGradient id="sw-top-sg" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%"   stop-color="#f0abfc"/>
            <stop offset="50%"  stop-color="#a855f7"/>
            <stop offset="100%" stop-color="#5b21b6"/>
          </linearGradient>
        </defs>
        <path d="M50,50 Q40,40 50,7  Q60,40 50,50Z" fill="url(#sw-top-sg)"/>
        <path d="M50,50 Q60,40 93,50 Q60,60 50,50Z" fill="url(#sw-top-sg)"/>
        <path d="M50,50 Q60,60 50,93 Q40,60 50,50Z" fill="url(#sw-top-sg)"/>
        <path d="M50,50 Q40,60 7,50  Q40,40 50,50Z" fill="url(#sw-top-sg)"/>
        <path d="M50,27 Q55,45 70,50 Q55,55 50,73 Q45,55 30,50 Q45,45 50,27Z" fill="white" opacity="0.9"/>
        <circle cx="50" cy="50" r="4.5" fill="white"/>
      </svg>
      <span class="top-logo-name">Nova Launcher</span>
    </div>
    {#if isRestart}
      <button class="close-btn" on:click={() => dispatch('close')}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
      </button>
    {/if}
  </div>

  <!-- Scrollable content -->
  <div class="content-area" class:fading={transitioning}>
    <div class="content-inner" class:wide={currentStep === 2}>

      <!-- Step 0: Welcome -->
      {#if currentStep === 0}
        <div class="welcome-wrap">
          <div class="welcome-logo" class:visible={showLogo}>
            <div class="welcome-logo-icon">
              <svg viewBox="0 0 100 100" width="100" height="100" xmlns="http://www.w3.org/2000/svg">
                <defs>
                  <linearGradient id="sw-wlc-sg" x1="0" y1="0" x2="1" y2="1">
                    <stop offset="0%"   stop-color="#f0abfc"/>
                    <stop offset="50%"  stop-color="#a855f7"/>
                    <stop offset="100%" stop-color="#5b21b6"/>
                  </linearGradient>
                </defs>
                <g class="sw-star">
                  <path class="sw-p sw-pn" d="M50,50 Q40,40 50,7  Q60,40 50,50Z" fill="url(#sw-wlc-sg)"/>
                  <path class="sw-p sw-pe" d="M50,50 Q60,40 93,50 Q60,60 50,50Z" fill="url(#sw-wlc-sg)"/>
                  <path class="sw-p sw-ps" d="M50,50 Q60,60 50,93 Q40,60 50,50Z" fill="url(#sw-wlc-sg)"/>
                  <path class="sw-p sw-pw" d="M50,50 Q40,60 7,50  Q40,40 50,50Z" fill="url(#sw-wlc-sg)"/>
                  <path class="sw-inner"
                    d="M50,27 Q55,45 70,50 Q55,55 50,73 Q45,55 30,50 Q45,45 50,27Z"
                    fill="white" opacity="0.9"/>
                  <circle class="sw-dot" cx="50" cy="50" r="4.5" fill="white"/>
                </g>
              </svg>
            </div>
          </div>
          <h1 class="welcome-h1" class:visible={showTitle}>Nova Launcher</h1>
          <p class="welcome-sub" class:visible={showSubtitle}>{lang.welcomeSubtitle}</p>
          <button class="btn-begin" class:visible={showStartBtn} on:click={startWizard} style="background:{accentHex}">
            {lang.startSetup}
            <svg viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2.5" width="16" height="16"><polyline points="9 18 15 12 9 6"/></svg>
          </button>
        </div>

      <!-- Step 1: Language -->
      {:else if currentStep === 1}
        <div class="step-wrap">
          <h2 class="step-h2">{lang.step1Title}</h2>
          <p class="step-hint">{lang.detected}: <strong style="color:{accentHex}">{languages.find(l=>l.code===selectedLanguage)?.label}</strong></p>
          <div class="lang-grid">
            {#each languages as l}
              <button class="lang-card" class:sel={selectedLanguage === l.code}
                style={selectedLanguage === l.code ? `border-color:${accentHex};box-shadow:0 0 0 1px ${accentHex}` : ''}
                on:click={() => selectedLanguage = l.code}>
                <span class="lang-flag">{l.flag}</span>
                <div class="lang-info"><span class="lang-name">{l.label}</span></div>
                {#if selectedLanguage === l.code}
                  <div class="lang-check" style="background:{accentHex}">
                    <svg viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2.5" width="12" height="12"><polyline points="20 6 9 17 4 12"/></svg>
                  </div>
                {/if}
              </button>
            {/each}
          </div>
        </div>

      <!-- Step 2: Design -->
      {:else if currentStep === 2}
        <div class="step-wrap design-step">
          <h2 class="step-h2">{lang.step2Title}</h2>
          <div class="design-cols">

            <!-- Left: controls -->
            <div class="design-controls">

              <!-- Accent Color -->
              <div class="wiz-card">
                <div class="wiz-section-title">{lang.accentColor}</div>
                <div class="color-picker-row">
                  <div class="color-picker-wrap">
                    <input type="color" bind:value={accentHex} class="native-color-input" />
                    <div class="color-swatch-big" style="background: {accentHex}">
                      <svg viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" width="14" height="14" opacity=".8"><path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>
                    </div>
                  </div>
                  <div class="color-info">
                    <span class="color-hex-label">{accentHex.toUpperCase()}</span>
                  </div>
                </div>
                <div class="color-presets-grid">
                  {#each colorPresets as p}
                    <button
                      class="preset-swatch"
                      class:active={accentHex.toLowerCase() === p.hex.toLowerCase()}
                      style="background: {p.hex}; --ring: {p.hex}"
                      title={lang[p.key] || p.key}
                      on:click={() => (accentHex = p.hex)}
                    ></button>
                  {/each}
                </div>
              </div>

              <!-- Theme -->
              <div class="wiz-card">
                <div class="wiz-section-title">{lang.theme}</div>
                <div class="theme-grid">
                  {#each themes as th}
                    <button class="theme-card" class:selected={uiTheme === th.id} on:click={() => (uiTheme = th.id)}>
                      <div class="theme-preview">
                        <div class="tp-bg" style="background: {th.bg}">
                          <div class="tp-sidebar" style="background: {th.surface}">
                            <div class="tp-nav-item" style="background: {accentHex}22; border-left: 2px solid {accentHex}"></div>
                            <div class="tp-nav-item2" style="background: {th.surface2}"></div>
                            <div class="tp-nav-item2" style="background: {th.surface2}"></div>
                          </div>
                          <div class="tp-main">
                            <div class="tp-bar" style="background: {th.surface}; border-bottom: 1px solid {th.border}"></div>
                            <div class="tp-content">
                              <div class="tp-card" style="background: {th.surface}; border: 1px solid {th.border}"></div>
                              <div class="tp-card" style="background: {th.surface}; border: 1px solid {th.border}; opacity:0.7"></div>
                            </div>
                          </div>
                        </div>
                      </div>
                      <span class="theme-label">{th.label}</span>
                      {#if uiTheme === th.id}
                        <div class="theme-check" style="background: {accentHex}">
                          <svg viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2.5" width="10" height="10"><polyline points="20 6 9 17 4 12"/></svg>
                        </div>
                      {/if}
                    </button>
                  {/each}
                </div>
              </div>

              <!-- Border Radius -->
              <div class="wiz-card">
                <div class="wiz-section-title">{lang.borderRadius}</div>
                <div class="radius-grid">
                  {#each radii as r}
                    <button class="radius-card" class:selected={uiRadius === r.id} on:click={() => (uiRadius = r.id)}>
                      <div class="radius-preview" style="border-radius: {r.preview}"></div>
                      <span class="radius-label">{lang[r.labelKey]}</span>
                    </button>
                  {/each}
                </div>
              </div>

            </div><!-- /design-controls -->

            <!-- Right: live preview -->
            <div class="design-preview-col">
              <div class="pvw-wrap">
                <div class="pvw-app" style="background:{previewTheme.bg};border-color:{previewTheme.border}">
                  <div class="pvw-sidebar" style="background:{previewTheme.surface};border-right:1px solid {previewTheme.border}">
                    <div class="pvw-logo-row">
                      <div class="pvw-logo-dot" style="background:{accentHex}"></div>
                      <div class="pvw-logo-text" style="background:{previewTheme.textMuted}44"></div>
                    </div>
                    <div class="pvw-nav">
                      <div class="pvw-nav-item pvw-active" style="background:{accentHex}18;border-left:2px solid {accentHex}">
                        <div class="pvw-nav-icon" style="background:{accentHex}"></div>
                        <div class="pvw-nav-text" style="background:{previewTheme.text}88"></div>
                      </div>
                      {#each [0,1,2,3] as _}
                        <div class="pvw-nav-item" style="border-left:2px solid transparent">
                          <div class="pvw-nav-icon" style="background:{previewTheme.textMuted}"></div>
                          <div class="pvw-nav-text" style="background:{previewTheme.textMuted}55"></div>
                        </div>
                      {/each}
                    </div>
                  </div>
                  <div class="pvw-main">
                    <div class="pvw-topbar" style="background:{previewTheme.surface};border-bottom:1px solid {previewTheme.border}">
                      <div class="pvw-topbar-title" style="background:{previewTheme.text}33"></div>
                      <div class="pvw-topbar-btn" style="background:{accentHex};border-radius:{previewRadius}"></div>
                    </div>
                    <div class="pvw-content">
                      <div class="pvw-card-grid">
                        {#each [0,1,2,3,4,5] as _}
                          <div class="pvw-instance" style="background:{previewTheme.surface};border:1px solid {previewTheme.border};border-radius:{previewRadius}">
                            <div class="pvw-instance-img" style="background:{previewTheme.surface2}"></div>
                            <div class="pvw-instance-footer">
                              <div class="pvw-instance-name" style="background:{previewTheme.text}44"></div>
                              <div class="pvw-play-btn" style="background:{accentHex};border-radius:{previewRadius}"></div>
                            </div>
                          </div>
                        {/each}
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div><!-- /design-preview-col -->

          </div><!-- /design-cols -->
        </div>

      <!-- Step 3: Discord -->
      {:else if currentStep === 3}
        <div class="step-wrap center-wrap">
          <div class="icon-circle" style="background:{accentHex}18;border-color:{accentHex}33">
            <svg viewBox="0 0 71 55" fill="none" xmlns="http://www.w3.org/2000/svg" width="44" height="44">
              <path d="M60.1 4.9A58.5 58.5 0 0 0 45.6.4a.2.2 0 0 0-.2.1 40.8 40.8 0 0 0-1.8 3.7 54 54 0 0 0-16.2 0 37.8 37.8 0 0 0-1.8-3.7.2.2 0 0 0-.2-.1A58.4 58.4 0 0 0 10.9 4.9a.2.2 0 0 0-.1.1C1.6 18.7-.9 32.2.3 45.5c0 .1.1.2.2.2a58.8 58.8 0 0 0 17.7 9 .2.2 0 0 0 .2-.1 42 42 0 0 0 3.6-5.9.2.2 0 0 0-.1-.3 38.7 38.7 0 0 1-5.5-2.6.2.2 0 0 1 0-.4c.4-.3.7-.6 1.1-.8a.2.2 0 0 1 .2 0c11.5 5.3 24 5.3 35.4 0a.2.2 0 0 1 .2 0l1.1.9a.2.2 0 0 1 0 .3 36.1 36.1 0 0 1-5.6 2.6.2.2 0 0 0-.1.3 47.1 47.1 0 0 0 3.6 5.9c.1.1.2.1.3.1a58.6 58.6 0 0 0 17.7-9 .2.2 0 0 0 .1-.2c1.5-15.4-2.6-28.8-10.8-40.6a.2.2 0 0 0-.1-.1ZM23.7 37.3c-3.5 0-6.4-3.2-6.4-7.2s2.8-7.2 6.4-7.2c3.6 0 6.5 3.3 6.4 7.2 0 3.9-2.8 7.2-6.4 7.2Zm23.6 0c-3.5 0-6.4-3.2-6.4-7.2s2.8-7.2 6.4-7.2c3.6 0 6.5 3.3 6.4 7.2 0 3.9-2.8 7.2-6.4 7.2Z" fill="#5865F2"/>
            </svg>
          </div>
          <h2 class="step-h2">{lang.step3Title}</h2>
          <p class="step-hint">{lang.step3Desc}</p>
          <div class="big-toggle-wrap">
            <button class="big-toggle" class:on={discordRpc} on:click={() => discordRpc=!discordRpc}
              style={discordRpc ? `background:${accentHex}` : ''} aria-pressed={discordRpc}>
              <span class="toggle-knob"></span>
            </button>
            <span class="toggle-label" style={discordRpc ? `color:${accentHex}` : ''}>{discordRpc ? lang.enabled : lang.disabled}</span>
          </div>
        </div>

      <!-- Step 4: Login -->
      {:else if currentStep === 4}
        <div class="step-wrap center-wrap">
          <div class="icon-circle" style="background:{accentHex}18;border-color:{accentHex}33">
            <svg viewBox="0 0 21 21" xmlns="http://www.w3.org/2000/svg" width="38" height="38">
              <rect x="1" y="1" width="9" height="9" fill="#f25022"/>
              <rect x="11" y="1" width="9" height="9" fill="#7fba00"/>
              <rect x="1" y="11" width="9" height="9" fill="#00a4ef"/>
              <rect x="11" y="11" width="9" height="9" fill="#ffb900"/>
            </svg>
          </div>
          <h2 class="step-h2">{lang.step4Title}</h2>
          {#if loginState === 'success'}
            <div class="login-ok">
              <div class="check-circle" style="border-color:{accentHex};background:{accentHex}22">
                <svg viewBox="0 0 24 24" fill="none" stroke={accentHex} stroke-width="2.5" width="28" height="28"><polyline points="20 6 9 17 4 12"/></svg>
              </div>
              <p style="color:{accentHex};font-weight:600;margin:8px 0 0">{lang.loginSuccess}</p>
            </div>
          {:else if loginState === 'idle' || loginState === 'error'}
            {#if loginState === 'error'}<p class="login-err">{loginError}</p>{/if}
            <button class="ms-btn" on:click={startBrowserLogin} style="border-color:{accentHex}50">
              <svg viewBox="0 0 21 21" xmlns="http://www.w3.org/2000/svg" width="18" height="18">
                <rect x="1" y="1" width="9" height="9" fill="#f25022"/>
                <rect x="11" y="1" width="9" height="9" fill="#7fba00"/>
                <rect x="1" y="11" width="9" height="9" fill="#00a4ef"/>
                <rect x="11" y="11" width="9" height="9" fill="#ffb900"/>
              </svg>
              {lang.loginBtn}
            </button>
          {:else}
            <div class="login-waiting">
              <div class="spinner" style="border-top-color:{accentHex}"></div>
              <span class="hint-text">{loginState==='opening' ? lang.loginOpening : lang.loginWaiting}</span>
            </div>
          {/if}
          <button class="skip-btn" on:click={goNext}>{lang.loginSkip}</button>
        </div>

      <!-- Step 5: Done -->
      {:else if currentStep === 5}
        <div class="step-wrap center-wrap">
          <div class="done-ring" style="border-color:{accentHex};background:{accentHex}18">
            <svg viewBox="0 0 24 24" fill="none" stroke={accentHex} stroke-width="2.5" width="52" height="52"><polyline points="20 6 9 17 4 12"/></svg>
          </div>
          <h2 class="step-h2 done-h2">{lang.step5Title}</h2>
          <p class="step-hint">{lang.doneSubtitle}</p>
          <button class="btn-done" style="background:{accentHex}" on:click={finishSetup}>
            {lang.beginPlay}
            <svg viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2.5" width="16" height="16"><polyline points="9 18 15 12 9 6"/></svg>
          </button>
        </div>
      {/if}

    </div><!-- content-inner -->
  </div><!-- content-area -->

  <!-- Navigation bar -->
  {#if currentStep >= 1 && currentStep <= 4}
    <div class="nav-bar">
      <button class="nav-back" on:click={goBack} disabled={currentStep <= 1}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="14" height="14"><polyline points="15 18 9 12 15 6"/></svg>
        {lang.back}
      </button>
      <div class="nav-dots">
        {#each [1,2,3,4,5] as s}
          <div class="nav-dot" class:cur={s===currentStep} class:done={s<currentStep}
            style={s===currentStep ? `background:${accentHex}` : s<currentStep ? `background:${accentHex}55` : ''}></div>
        {/each}
      </div>
      <button class="nav-next" style="background:{accentHex}" on:click={goNext}
        disabled={currentStep===4 && loginState!=='idle' && loginState!=='error' && loginState!=='success'}>
        {lang.next}
        <svg viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2.5" width="14" height="14"><polyline points="9 18 15 12 9 6"/></svg>
      </button>
    </div>
  {/if}

</div>

<style>
  /* ── Root layout ── */
  .wizard-root {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: #080810;
    overflow: hidden;
    font-family: inherit;
  }

  /* ── Top bar ── */
  .top-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 24px;
    border-bottom: 1px solid rgba(255,255,255,0.05);
    flex-shrink: 0;
  }
  .top-logo {
    display: flex;
    align-items: center;
    gap: 9px;
  }
  .top-logo-name {
    font-size: 0.88rem;
    font-weight: 700;
    color: rgba(255,255,255,0.7);
    letter-spacing: -0.01em;
  }
  .close-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: 7px;
    border: 1px solid rgba(255,255,255,0.1);
    background: transparent;
    color: rgba(255,255,255,0.35);
    cursor: pointer;
  }
  .close-btn:hover {
    background: rgba(255,255,255,0.07);
    color: rgba(255,255,255,0.7);
  }

  /* ── Scrollable content ── */
  .content-area {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    opacity: 1;
    transition: opacity 0.25s ease;
  }
  .content-area.fading { opacity: 0; }

  .content-inner {
    max-width: 640px;
    margin: 0 auto;
    padding: 48px 32px 36px;
  }
  .content-inner.wide {
    max-width: 920px;
  }

  /* ── Welcome step ── */
  .welcome-wrap {
    height: 100%;
    min-height: 380px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 18px;
    text-align: center;
  }
  .welcome-logo {
    opacity: 0;
    transition: opacity 0.6s ease;
  }
  .welcome-logo.visible {
    opacity: 1;
  }
  .welcome-logo-icon {
    width: 108px;
    height: 108px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 24px;
    background: rgba(255,255,255,0.04);
    box-shadow: 0 8px 48px rgba(0,0,0,0.45);
  }
  .sw-p { transform-box: fill-box; animation: swPetalIn 0.5s cubic-bezier(0.34,1.56,0.64,1) both; }
  .sw-pn { transform-origin: center bottom; animation-delay: 0.00s; }
  .sw-pe { transform-origin: left   center; animation-delay: 0.08s; }
  .sw-ps { transform-origin: center top;    animation-delay: 0.16s; }
  .sw-pw { transform-origin: right  center; animation-delay: 0.24s; }
  @keyframes swPetalIn { 0% { opacity: 0; transform: scale(0); } 100% { opacity: 1; transform: scale(1); } }
  .sw-inner { transform-box: fill-box; transform-origin: center; animation: swInnerIn 0.3s ease-out 0.4s both; }
  @keyframes swInnerIn { from { opacity: 0; transform: scale(0.15); } to { opacity: 0.9; transform: scale(1); } }
  .sw-dot { transform-box: fill-box; transform-origin: center; animation: swDotIn 0.25s ease-out 0.6s both; }
  @keyframes swDotIn { from { opacity: 0; transform: scale(0); } to { opacity: 1; transform: scale(1); } }
  .sw-star { transform-box: fill-box; transform-origin: center; animation: swStarPulse 2.6s ease-in-out 0.8s infinite; }
  @keyframes swStarPulse { 0%, 100% { transform: scale(1); opacity: 1; } 50% { transform: scale(0.93); opacity: 0.8; } }
  .welcome-h1 {
    font-size: 2.8rem;
    font-weight: 800;
    color: #f0eaff;
    letter-spacing: -0.03em;
    margin: 0;
    opacity: 0;
    transition: opacity 0.55s ease 0.15s;
  }
  .welcome-h1.visible { opacity: 1; }

  .welcome-sub {
    font-size: 1rem;
    color: rgba(240,234,255,0.45);
    margin: 0;
    opacity: 0;
    transition: opacity 0.55s ease 0.28s;
  }
  .welcome-sub.visible { opacity: 1; }

  .btn-begin {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 13px 30px;
    border-radius: 10px;
    border: none;
    color: #fff;
    font-size: 0.95rem;
    font-weight: 600;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.5s ease 0.42s;
    box-shadow: 0 4px 24px rgba(0,0,0,0.35);
    margin-top: 4px;
  }
  .btn-begin.visible { opacity: 1; }
  .btn-begin:hover { opacity: 0.86; }

  /* ── Generic step wrapper ── */
  .step-wrap {
    display: flex;
    flex-direction: column;
    gap: 22px;
    height: 100%;
  }
  .step-h2 {
    font-size: 1.65rem;
    font-weight: 700;
    color: #f0eaff;
    letter-spacing: -0.02em;
    margin: 0;
  }
  .step-hint {
    font-size: 0.88rem;
    color: rgba(240,234,255,0.45);
    margin: -10px 0 0;
  }

  /* ── Language grid ── */
  .lang-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
  }
  .lang-card {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 11px 14px;
    border-radius: 9px;
    border: 1.5px solid rgba(255,255,255,0.07);
    background: rgba(255,255,255,0.025);
    color: rgba(255,255,255,0.55);
    font-size: 0.86rem;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    position: relative;
  }
  .lang-card:hover {
    border-color: rgba(255,255,255,0.18);
    background: rgba(255,255,255,0.055);
    color: rgba(255,255,255,0.88);
  }
  .lang-card.sel {
    color: #fff;
    background: rgba(255,255,255,0.04);
  }
  .lang-flag { font-size: 1.25rem; flex-shrink: 0; }
  .lang-info { flex: 1; }
  .lang-name { font-weight: 600; }
  .lang-check {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  /* ── Design step ── */
  .design-step { gap: 18px; }
  .design-cols {
    display: grid;
    grid-template-columns: 1fr 260px;
    gap: 18px;
    align-items: start;
  }
  .design-controls {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .wiz-card {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    background: var(--surface, rgba(255,255,255,0.03));
    border: 1px solid var(--border, rgba(255,255,255,0.07));
    border-radius: var(--radius, 8px);
  }
  .wiz-section-title {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-muted, rgba(255,255,255,0.3));
  }

  /* Color picker (matches Settings) */
  .color-picker-row { display: flex; align-items: flex-start; gap: 14px; }
  .color-picker-wrap { position: relative; width: 52px; height: 52px; flex-shrink: 0; cursor: pointer; border-radius: var(--radius, 8px); overflow: hidden; box-shadow: 0 4px 16px rgba(0,0,0,0.35); }
  .native-color-input { position: absolute; inset: 0; width: 100%; height: 100%; opacity: 0; cursor: pointer; border: none; padding: 0; }
  .color-swatch-big { width: 52px; height: 52px; display: flex; align-items: center; justify-content: center; pointer-events: none; }
  .color-info { display: flex; flex-direction: column; gap: 4px; justify-content: center; }
  .color-hex-label { font-size: 18px; font-weight: 700; font-family: monospace; color: var(--text, #f0eaff); letter-spacing: 0.05em; }
  .color-presets-grid { display: grid; grid-template-columns: repeat(16, 1fr); gap: 5px; }
  .preset-swatch { width: 18px; height: 18px; border-radius: 50%; border: 2px solid transparent; cursor: pointer; transition: all 0.12s ease; flex-shrink: 0; }
  .preset-swatch:hover { transform: scale(1.25); }
  .preset-swatch.active { border-color: white; box-shadow: 0 0 0 2px var(--ring, var(--accent, #a855f7)); transform: scale(1.2); }

  /* Theme picker (matches Settings) */
  .theme-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; }
  .theme-card { display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 8px 6px; border-radius: var(--radius, 8px); border: 1px solid var(--border, rgba(255,255,255,0.08)); background: var(--surface2, rgba(255,255,255,0.04)); cursor: pointer; transition: all 0.15s ease; position: relative; }
  .theme-card:hover { border-color: rgba(var(--accent-rgb, 168,85,247),0.4); transform: translateY(-1px); }
  .theme-card.selected { border-color: var(--accent, #a855f7); background: rgba(var(--accent-rgb, 168,85,247),0.08); box-shadow: 0 0 0 1px rgba(var(--accent-rgb, 168,85,247),0.25); }
  .theme-preview { width: 100%; height: 48px; border-radius: 4px; overflow: hidden; }
  .tp-bg { width: 100%; height: 100%; display: flex; }
  .tp-sidebar { width: 30%; height: 100%; padding: 4px 3px; display: flex; flex-direction: column; gap: 2px; }
  .tp-nav-item { height: 6px; border-radius: 1px; }
  .tp-nav-item2 { height: 4px; border-radius: 1px; }
  .tp-main { flex: 1; display: flex; flex-direction: column; }
  .tp-bar { height: 10px; }
  .tp-content { flex: 1; padding: 3px; display: flex; flex-direction: column; gap: 2px; }
  .tp-card { flex: 1; border-radius: 2px; }
  .theme-label { font-size: 10px; font-weight: 500; color: var(--text-dim, rgba(255,255,255,0.55)); }
  .theme-card.selected .theme-label { color: var(--accent, #a855f7); }
  .theme-check { position: absolute; top: 4px; right: 4px; width: 14px; height: 14px; border-radius: 50%; display: flex; align-items: center; justify-content: center; }

  /* Radius picker (matches Settings) */
  .radius-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; }
  .radius-card { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 12px 8px; border-radius: var(--radius, 8px); border: 1px solid var(--border, rgba(255,255,255,0.08)); background: var(--surface2, rgba(255,255,255,0.04)); cursor: pointer; transition: all 0.15s ease; }
  .radius-card:hover { border-color: rgba(var(--accent-rgb, 168,85,247),0.4); transform: translateY(-1px); }
  .radius-card.selected { border-color: var(--accent, #a855f7); background: rgba(var(--accent-rgb, 168,85,247),0.08); box-shadow: 0 0 0 1px rgba(var(--accent-rgb, 168,85,247),0.25); }
  .radius-preview { width: 34px; height: 34px; background: var(--surface, rgba(255,255,255,0.06)); border: 1px solid var(--border, rgba(255,255,255,0.12)); }
  .radius-label { font-size: 10px; font-weight: 500; color: var(--text-dim, rgba(255,255,255,0.55)); }
  .radius-card.selected .radius-label { color: var(--accent, #a855f7); }

  /* Live preview panel */
  .design-preview-col {
    position: sticky;
    top: 0;
  }
  .pvw-wrap {
    border-radius: var(--radius, 8px);
    overflow: hidden;
    border: 1px solid rgba(255,255,255,0.08);
    box-shadow: 0 8px 32px rgba(0,0,0,0.45);
  }
  .pvw-app {
    display: flex;
    height: 340px;
    border-radius: inherit;
    overflow: hidden;
  }
  .pvw-sidebar {
    width: 72px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 7px;
    flex-shrink: 0;
  }
  .pvw-logo-row {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-bottom: 6px;
  }
  .pvw-logo-dot { width: 14px; height: 14px; border-radius: 4px; flex-shrink: 0; }
  .pvw-logo-text { height: 5px; flex: 1; border-radius: 3px; }
  .pvw-nav { display: flex; flex-direction: column; gap: 3px; }
  .pvw-nav-item { display: flex; align-items: center; gap: 5px; padding: 5px 5px; border-radius: 4px; }
  .pvw-nav-icon { width: 9px; height: 9px; border-radius: 2px; flex-shrink: 0; }
  .pvw-nav-text { height: 4px; flex: 1; border-radius: 3px; }
  .pvw-main { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
  .pvw-topbar { display: flex; align-items: center; justify-content: space-between; padding: 7px 10px; gap: 8px; flex-shrink: 0; }
  .pvw-topbar-title { height: 5px; width: 60px; border-radius: 3px; }
  .pvw-topbar-btn { height: 16px; width: 40px; border-radius: 4px; }
  .pvw-content { flex: 1; padding: 8px; overflow: hidden; }
  .pvw-card-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 5px; height: 100%; }
  .pvw-instance { display: flex; flex-direction: column; overflow: hidden; }
  .pvw-instance-img { flex: 1; }
  .pvw-instance-footer { display: flex; align-items: center; justify-content: space-between; padding: 4px 5px; }
  .pvw-instance-name { height: 4px; width: 30px; border-radius: 3px; }
  .pvw-play-btn { width: 16px; height: 10px; }

  /* ── Centered steps (Discord / Login / Done) ── */
  .center-wrap {
    align-items: center;
    justify-content: center;
    text-align: center;
    min-height: 380px;
    gap: 16px;
  }
  .icon-circle {
    width: 78px;
    height: 78px;
    border-radius: 50%;
    border: 1.5px solid;
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 6px;
  }

  /* ── Discord toggle ── */
  .big-toggle-wrap {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-top: 8px;
  }
  .big-toggle {
    width: 58px;
    height: 30px;
    border-radius: 15px;
    background: rgba(255,255,255,0.1);
    border: none;
    cursor: pointer;
    position: relative;
    padding: 0;
    flex-shrink: 0;
  }
  .toggle-knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: rgba(255,255,255,0.35);
    display: block;
  }
  .big-toggle.on .toggle-knob {
    left: 31px;
    background: #fff;
  }
  .toggle-label {
    font-size: 0.9rem;
    font-weight: 600;
    color: rgba(255,255,255,0.45);
  }

  /* ── Login step ── */
  .ms-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 24px;
    border-radius: 9px;
    border: 1.5px solid;
    background: transparent;
    color: rgba(255,255,255,0.82);
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
  }
  .ms-btn:hover { background: rgba(255,255,255,0.05); }

  .login-ok {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
  }
  .check-circle {
    width: 60px;
    height: 60px;
    border-radius: 50%;
    border: 2px solid;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .login-err {
    color: #f87171;
    font-size: 0.83rem;
    background: rgba(248,113,113,0.07);
    border: 1px solid rgba(248,113,113,0.18);
    border-radius: 8px;
    padding: 8px 14px;
    max-width: 320px;
  }
  .login-waiting {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }
  .spinner {
    width: 28px;
    height: 28px;
    border: 2.5px solid rgba(255,255,255,0.1);
    border-top-color: currentColor;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg) } }
  .hint-text {
    font-size: 0.82rem;
    color: rgba(255,255,255,0.38);
  }
  .skip-btn {
    font-size: 0.8rem;
    color: rgba(255,255,255,0.28);
    background: none;
    border: none;
    cursor: pointer;
    padding: 4px 10px;
    border-radius: 5px;
    margin-top: 6px;
  }
  .skip-btn:hover { color: rgba(255,255,255,0.52); }

  /* ── Done step ── */
  .done-ring {
    width: 88px;
    height: 88px;
    border-radius: 50%;
    border: 2px solid;
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 6px;
  }
  .done-h2 { font-size: 2rem; }
  .btn-done {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 13px 30px;
    border-radius: 10px;
    border: none;
    color: #fff;
    font-size: 0.92rem;
    font-weight: 700;
    cursor: pointer;
    margin-top: 8px;
    box-shadow: 0 4px 20px rgba(0,0,0,0.3);
  }
  .btn-done:hover { opacity: 0.87; }

  /* ── Navigation bar ── */
  .nav-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 24px 18px;
    border-top: 1px solid rgba(255,255,255,0.05);
    flex-shrink: 0;
  }
  .nav-back {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    border-radius: 7px;
    border: 1px solid rgba(255,255,255,0.09);
    background: transparent;
    color: rgba(255,255,255,0.38);
    font-size: 0.82rem;
    font-weight: 500;
    cursor: pointer;
  }
  .nav-back:hover:not(:disabled) {
    border-color: rgba(255,255,255,0.2);
    color: rgba(255,255,255,0.7);
  }
  .nav-back:disabled {
    opacity: 0.22;
    cursor: not-allowed;
  }
  .nav-dots {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
  }
  .nav-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgba(255,255,255,0.11);
  }
  .nav-dot.cur {
    width: 20px;
    border-radius: 4px;
  }
  .nav-dot.done { opacity: 0.55; }
  .nav-next {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 18px;
    border-radius: 7px;
    border: none;
    color: #fff;
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    box-shadow: 0 2px 10px rgba(0,0,0,0.28);
  }
  .nav-next:hover:not(:disabled) { opacity: 0.87; }
  .nav-next:disabled {
    opacity: 0.28;
    cursor: not-allowed;
  }
</style>
