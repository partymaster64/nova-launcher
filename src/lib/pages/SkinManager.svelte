<script>
  import { onMount, onDestroy } from 'svelte'
  import { get } from 'svelte/store'
  import { invoke } from '@tauri-apps/api/core'
  import { accounts, config, addToast, isOnline } from '../../store.js'
  import * as skinview3d from 'skinview3d'
  import { t } from '../i18n.js'

  let presets = []
  let activePresetId = null
  let selectedPreset = null
  let viewerCanvas
  let viewerArea
  let shadowEl
  let viewer = null
  let fileInput
  let resizeObserver = null

  let loadingCurrent = true
  let applying = false
  let currentSkinInfo = null

  // Modal state
  let showModal = false
  let modalStep = 1
  let editingPreset = null
  let modalName = ''
  let modalModel = 'classic'
  let modalSkinData = null
  let modalCapeId = null
  let modalCapeData = null
  let modalPreviewCanvas
  let saving = false
  let deleting = null
  let confirmDeleteId = null
  let dragging = false

  // Cape picker
  let availableCapes = []
  let loadingCapes = false

  let cfg = null
  let accs = []
  config.subscribe(v => (cfg = v))
  accounts.subscribe(v => (accs = v))

  $: activeAccount = accs.find(a => a.uuid === cfg?.active_account_uuid)

  // ─── skinview3d 3D viewer ────────────────────────────────────────────────
  // Animation blend system — one master animation, smoothstep lerp between poses

  function mkShadow() {
    const p = () => ({ rotation: { x: 0, y: 0, z: 0 } })
    return {
      position: { x: 0, y: 0 },
      rotation: { x: 0, y: 0, z: 0 },
      skin: { head: p(), body: p(), leftArm: p(), rightArm: p(), leftLeg: p(), rightLeg: p() },
      cape: { rotation: { x: 0 } }
    }
  }
  const PARTS = ['head','body','leftArm','rightArm','leftLeg','rightLeg']
  const ss  = t => t <= 0 ? 0 : t >= 1 ? 1 : t * t * (3 - 2 * t)   // smoothstep
  const lv  = (a, b, t) => a + (b - a) * t

  // ── Animation functions (write into shadow players) ──────────────────────

const BreathingAnimation = (pl, time) => {
    const sk = pl.skin
    const breath  = Math.sin(time * 1.8)           // chest rise ~1.8 rad/s
    const ws      = Math.sin(time * 0.55)           // slow weight shift
    const look    = Math.sin(time * 0.38)           // idle head wander
    const legAlt  = Math.sin(time * 0.4) * 0.05    // subtle alternating foot shift

    pl.position.y          =  0                    // feet stay on ground
    sk.body.rotation.x     =  breath * 0.025
    sk.body.rotation.z     =  ws * 0.02            // hip sway
    sk.head.rotation.x     = -breath * 0.025 + look * 0.06
    sk.head.rotation.y     =  Math.sin(time * 0.42) * 0.12
    sk.leftArm.rotation.z  =  Math.PI * 0.06 + breath * 0.04 + ws * 0.03
    sk.rightArm.rotation.z = -Math.PI * 0.06 - breath * 0.04 - ws * 0.03
    sk.leftArm.rotation.x  =  breath * 0.03
    sk.rightArm.rotation.x =  breath * 0.03
    sk.leftLeg.rotation.z  =  ws * 0.055
    sk.rightLeg.rotation.z =  ws * 0.055
    sk.leftLeg.rotation.x  =  legAlt              // legs alternate gently forward/back
    sk.rightLeg.rotation.x = -legAlt
    pl.cape.rotation.x     =  Math.PI * 0.07 + breath * 0.04
  }

  const WaveEmote = (pl, time) => {
    const sk = pl.skin
    const breath  = Math.sin(time * 1.8)
    const ws      = Math.sin(time * 0.55)
    const legAlt  = Math.sin(time * 0.4) * 0.05

    pl.position.y          =  0
    sk.body.rotation.x     =  breath * 0.02
    sk.body.rotation.z     =  ws * 0.02
    sk.head.rotation.y     =  0.18                 // turned toward wave
    sk.head.rotation.x     = -0.04
    // Right arm: raised and waving fast
    sk.rightArm.rotation.x = -Math.PI * 0.75
    sk.rightArm.rotation.z =  Math.sin(time * 8.5) * 0.50 - Math.PI * 0.20
    // Left arm: natural hang
    sk.leftArm.rotation.z  =  Math.PI * 0.06 + breath * 0.04
    sk.leftArm.rotation.x  =  breath * 0.03
    // Legs: planted, slight alternating shift
    sk.leftLeg.rotation.z  =  ws * 0.055
    sk.rightLeg.rotation.z =  ws * 0.055
    sk.leftLeg.rotation.x  =  legAlt
    sk.rightLeg.rotation.x = -legAlt
    pl.cape.rotation.x     =  Math.PI * 0.08 + breath * 0.04
  }

  const LookAroundEmote = (pl, time) => {
    const sk = pl.skin
    const breath = Math.sin(time * 1.8)
    const turn   = Math.sin(time * 1.3)            // head sweep
    const ws     = Math.sin(time * 0.55)

    pl.position.y          =  0
    sk.body.rotation.x     =  breath * 0.02
    sk.body.rotation.y     =  turn * 0.09           // body follows head subtly
    sk.head.rotation.y     =  turn * 0.88
    sk.head.rotation.x     =  Math.sin(time * 0.9) * 0.14 - breath * 0.02
    sk.leftArm.rotation.z  =  Math.PI * 0.06 + breath * 0.04 + ws * 0.03
    sk.rightArm.rotation.z = -Math.PI * 0.06 - breath * 0.04 - ws * 0.03
    sk.leftArm.rotation.x  =  breath * 0.03
    sk.rightArm.rotation.x =  breath * 0.03
    // Legs: planted, slight alternating shift as body rotates
    const legAlt = Math.sin(time * 0.4) * 0.05
    sk.leftLeg.rotation.z  =  ws * 0.055
    sk.rightLeg.rotation.z =  ws * 0.055
    sk.leftLeg.rotation.x  =  legAlt
    sk.rightLeg.rotation.x = -legAlt
    pl.cape.rotation.x     =  Math.PI * 0.07 + breath * 0.04
  }

  const CheerEmote = (pl, time) => {
    const sk = pl.skin
    // Hop: sharp upward push, soft landing
    const hop    = Math.max(0, Math.sin(time * 5.0)) * 1.1
    // Legs alternate forward/back as body bounces — one kicks forward, one back
    const legAlt = Math.sin(time * 5.0) * 0.22

    pl.position.y          =  hop
    sk.body.rotation.x     = -hop * 0.06            // slight lean back when airborne
    sk.head.rotation.x     = -hop * 0.04
    // Arms pump upward together with excitement
    sk.leftArm.rotation.x  = -Math.PI * 0.55 - Math.sin(time * 5.0) * 0.30
    sk.rightArm.rotation.x = -Math.PI * 0.55 - Math.sin(time * 5.0) * 0.30
    sk.leftArm.rotation.z  =  Math.PI * 0.13
    sk.rightArm.rotation.z = -Math.PI * 0.13
    // Legs: alternate forward/back (not both same = mermaid), slight spread when airborne
    sk.leftLeg.rotation.x  =  legAlt
    sk.rightLeg.rotation.x = -legAlt
    sk.leftLeg.rotation.z  =  hop * 0.10             // spread outward
    sk.rightLeg.rotation.z = -hop * 0.10             // spread outward
    pl.cape.rotation.x     =  Math.PI * 0.10 + hop * 0.10
  }

  const emotes = [
    { anim: WaveEmote,       duration: 3000 },
    { anim: LookAroundEmote, duration: 3500 },
    { anim: CheerEmote,      duration: 2800 },
  ]

  // ── Blend state ──────────────────────────────────────────────────────────

  let fromFn     = null
  let toFn       = BreathingAnimation
  let fromOffset = 0
  let toOffset   = 0
  let blendProg  = 1.0   // start fully on idle
  let lastT      = -1
  const BLEND    = 0.4   // seconds

  // Single master animation registered once with skinview3d
  const MasterAnimation = (player, time) => {
    const dt = lastT < 0 ? 0 : time - lastT
    lastT = time
    blendProg = Math.min(blendProg + dt / BLEND, 1.0)
    const t = ss(blendProg)

    const A = mkShadow(), B = mkShadow()
    if (fromFn) fromFn(A, time - fromOffset)
    if (toFn)   toFn(B,   time - toOffset)

    player.position.y = lv(A.position.y, B.position.y, t)
    for (const part of PARTS) {
      for (const ax of ['x','y','z']) {
        player.skin[part].rotation[ax] = lv(
          A.skin[part].rotation[ax],
          B.skin[part].rotation[ax], t)
      }
    }
    if (player.cape) player.cape.rotation.x = lv(A.cape.rotation.x, B.cape.rotation.x, t)

    // Sync CSS shadow: scale down + fade as player hops up
    if (shadowEl) {
      const hop = Math.max(0, player.position.y)
      const sc = Math.max(0.35, 1 - hop * 0.28)
      shadowEl.style.opacity  = String(Math.max(0.04, 0.38 - hop * 0.22))
      shadowEl.style.transform = `translateX(-50%) scaleX(${sc})`
    }
  }

  function switchAnim(fn) {
    if (fn === toFn && blendProg >= 1.0) return
    fromFn     = toFn
    fromOffset = toOffset
    toFn       = fn
    toOffset   = lastT >= 0 ? lastT : 0
    blendProg  = 0.0
  }

  let animTimer  = null
  let emoteTimer = null
  let inEmote    = false

  function playIdle()  { switchAnim(BreathingAnimation) }

  function playRandomEmote() {
    if (!viewer || inEmote) return
    inEmote = true
    const emote = emotes[Math.floor(Math.random() * emotes.length)]
    switchAnim(emote.anim)
    animTimer = setTimeout(() => {
      inEmote = false
      playIdle()
      scheduleNextEmote()
    }, emote.duration)
  }

  function scheduleNextEmote() {
    clearTimeout(emoteTimer)
    emoteTimer = setTimeout(playRandomEmote, 30000)
  }

  async function initViewer() {
    if (!viewerCanvas || viewer) return
    try {
      const w = viewerArea?.clientWidth  || 300
      const h = viewerArea?.clientHeight || 480
      viewer = new skinview3d.SkinViewer({
        canvas: viewerCanvas,
        width: w,
        height: h,
      })
      viewer.fov = 65
      viewer.zoom = 0.82        // slightly zoomed out so model has lateral room
      viewer.renderer.setClearColor(0x000000, 0)

      const controls = skinview3d.createOrbitControls(viewer)
      controls.enableZoom = false
      controls.enablePan = false
      controls.minPolarAngle = Math.PI / 2
      controls.maxPolarAngle = Math.PI / 2

      viewer.animations.add(MasterAnimation)
      playIdle()
      scheduleNextEmote()

      // Resize viewer whenever the container changes size
      resizeObserver = new ResizeObserver(entries => {
        for (const entry of entries) {
          const { width, height } = entry.contentRect
          if (viewer && width > 0 && height > 0) {
            viewer.width  = width
            viewer.height = height
          }
        }
      })
      if (viewerArea) resizeObserver.observe(viewerArea)

      if (selectedPreset) await applyPresetToViewer(selectedPreset)
    } catch (e) {
      console.error('skinview3d init failed:', e)
    }
  }

  async function applyPresetToViewer(preset) {
    if (!viewer) return
    try {
      await viewer.loadSkin(`data:image/png;base64,${preset.skin_data}`)
      viewer.playerObject.skin.slim = preset.model === 'slim'
      if (preset.cape_data) {
        await viewer.loadCape(`data:image/png;base64,${preset.cape_data}`)
      } else {
        viewer.loadCape(null)
      }
    } catch (e) { console.error('skin load failed:', e) }
  }

  onMount(async () => {
    await loadPresets()
    await initViewer()
    loadCurrentSkin()
    loadCapes()   // load early so cape names appear in cards
  })

  onDestroy(() => {
    clearTimeout(animTimer)
    clearTimeout(emoteTimer)
    resizeObserver?.disconnect()
    viewer?.dispose()
    viewer = null
  })

  // ─── Presets ─────────────────────────────────────────────────────────────

  async function loadPresets() {
    try {
      const result = await invoke('get_skin_presets')
      presets = result.presets
      activePresetId = result.active_id
      const sel = presets.find(p => p.id === activePresetId) || presets[0] || null
      if (sel) {
        selectedPreset = sel
        await applyPresetToViewer(sel)
      }
    } catch (e) { console.error(e) }
  }

  function isRateLimit(e) {
    return String(e).includes('RATE_LIMIT')
  }

  async function selectPreset(preset) {
    selectedPreset = preset
    await applyPresetToViewer(preset)
    applying = true
    try {
      await invoke('apply_skin_to_mojang', { id: preset.id })
      activePresetId = preset.id
      addToast(`Skin "${preset.name}" auf Mojang angewendet`, 'success')
      currentSkinInfo = await invoke('get_current_mojang_skin').catch(() => currentSkinInfo)
    } catch (e) {
      if (isRateLimit(e)) addToast(get(t)('skinManager.rateLimit'), 'error')
      else addToast(String(e), 'error')
    } finally {
      applying = false
    }
  }

  function requestDelete(preset) {
    confirmDeleteId = preset.id
  }

  function cancelDelete() {
    confirmDeleteId = null
  }

  async function confirmDelete(preset) {
    deleting = preset.id
    confirmDeleteId = null
    try {
      await invoke('delete_skin_preset', { id: preset.id })
      if (selectedPreset?.id === preset.id) {
        selectedPreset = null
        viewer?.loadSkin(null)
      }
      await loadPresets()
    } catch (e) { console.error(e) } finally { deleting = null }
  }

  // ─── Load current Mojang skin (read-only) ────────────────────────────────

  async function loadCurrentSkin() {
    if (!activeAccount) { loadingCurrent = false; return }
    loadingCurrent = true
    try {
      currentSkinInfo = await invoke('get_current_mojang_skin')
      if (currentSkinInfo.matched_preset_id) {
        const match = presets.find(p => p.id === currentSkinInfo.matched_preset_id)
        if (match) { selectedPreset = match; activePresetId = match.id }
      } else {
        try {
          const preset = await invoke('import_current_skin_as_preset')
          await loadPresets()
          const imported = presets.find(p => p.id === preset.id) || preset
          selectedPreset = imported
          activePresetId = imported.id
        } catch (_) {}
      }
      if (viewer) {
        await viewer.loadSkin(`data:image/png;base64,${currentSkinInfo.skin_data}`)
        viewer.playerObject.skin.slim = currentSkinInfo.model === 'slim'
        if (currentSkinInfo.cape_data) {
          await viewer.loadCape(`data:image/png;base64,${currentSkinInfo.cape_data}`)
        } else {
          viewer.loadCape(null)
        }
      }
    } catch (e) {
      if (isRateLimit(e)) addToast(get(t)('skinManager.rateLimit'), 'error')
    }
    loadingCurrent = false
  }

  // ─── Slim auto-detection ─────────────────────────────────────────────────

  function detectSlim(base64) {
    return new Promise(resolve => {
      const img = new Image()
      img.onload = () => {
        if (img.height < 64) { resolve(false); return }
        const c = document.createElement('canvas')
        c.width = img.width; c.height = img.height
        const ctx = c.getContext('2d')
        ctx.drawImage(img, 0, 0)
        resolve(ctx.getImageData(54, 20, 1, 1).data[3] < 128)
      }
      img.onerror = () => resolve(false)
      img.src = `data:image/png;base64,${base64}`
    })
  }

  // ─── Cape front-face canvas action ───────────────────────────────────────

  function capeFront(node, { capeData }) {
    function draw(data) {
      if (!data) return
      const img = new Image()
      img.onload = () => {
        node.width = 10
        node.height = 16
        const ctx = node.getContext('2d')
        ctx.imageSmoothingEnabled = false
        ctx.clearRect(0, 0, 10, 16)
        ctx.drawImage(img, 1, 1, 10, 16, 0, 0, 10, 16)
      }
      img.src = `data:image/png;base64,${data}`
    }
    draw(capeData)
    return { update({ capeData: d }) { draw(d) } }
  }

  // ─── Skin upper-body canvas (for preset cards) ────────────────────────────
  // Renders head + torso + arms at scale 6

  function skinUpper(node, { skinData, model }) {
    function draw(data, m) {
      if (!data) return
      const img = new Image()
      img.onload = () => {
        const armW = m === 'slim' ? 3 : 4
        const W = 8 + armW * 2
        const H = 20   // head(8) + torso(12)
        const scale = 6
        node.width = W * scale
        node.height = H * scale
        const ctx = node.getContext('2d')
        ctx.imageSmoothingEnabled = false
        ctx.clearRect(0, 0, node.width, node.height)
        const cx = armW
        const b = (sx, sy, sw, sh, dx, dy) =>
          ctx.drawImage(img, sx, sy, sw, sh, dx*scale, dy*scale, sw*scale, sh*scale)
        b(8,8,  8,8,    cx,0);    b(40,8,    8,8,    cx,0)    // head base + overlay
        b(20,20, 8,12,  cx,8);    b(20,36,   8,12,   cx,8)    // torso base + overlay
        b(44,20, armW,12, cx-armW,8); b(44,36, armW,12, cx-armW,8) // R arm
        b(36,52, armW,12, cx+8,8);    b(52,52, armW,12, cx+8,8)   // L arm
      }
      img.src = `data:image/png;base64,${data}`
    }
    draw(skinData, model)
    return { update({ skinData: d, model: m }) { draw(d, m) } }
  }

  // ─── Modal ────────────────────────────────────────────────────────────────

  async function loadCapes() {
    if (!activeAccount || availableCapes.length > 0) return
    loadingCapes = true
    try { availableCapes = await invoke('get_owned_capes') } catch (e) {
      if (isRateLimit(e)) addToast(get(t)('skinManager.rateLimit'), 'error')
    }
    loadingCapes = false
  }

  function openAddModal() {
    if (viewer) viewer.renderPaused = true
    editingPreset = null
    modalStep = 1
    modalName = get(t)('skinManager.presetNamePlaceholder')
    modalModel = 'classic'
    modalSkinData = null
    modalCapeId = null
    modalCapeData = null
    dragging = false
    showModal = true
    loadCapes()
  }

  function openEditModal(preset) {
    if (viewer) viewer.renderPaused = true
    editingPreset = preset
    modalStep = 2
    modalName = preset.name
    modalModel = preset.model
    modalSkinData = preset.skin_data
    modalCapeId = preset.cape_id || null
    modalCapeData = preset.cape_data || null
    showModal = true
    setTimeout(() => drawModalPreview(), 60)
    loadCapes()
  }

  function closeModal() {
    showModal = false
    editingPreset = null
    if (viewer) viewer.renderPaused = false
  }

  // ─── Drop zone ────────────────────────────────────────────────────────────

  function onDragOver(e) { e.preventDefault(); dragging = true }
  function onDragLeave() { dragging = false }
  async function onDrop(e) {
    e.preventDefault(); dragging = false
    const file = e.dataTransfer?.files[0]
    if (file?.type === 'image/png') await loadSkinFile(file)
  }
  async function loadSkinFile(file) {
    const data = await readFileAsBase64(file)
    modalSkinData = data
    modalModel = (await detectSlim(data)) ? 'slim' : 'classic'
    modalName = file.name.replace(/\.png$/i, '') || get(t)('skinManager.presetNamePlaceholder')
    modalStep = 2
    setTimeout(() => drawModalPreview(), 60)
  }

  async function saveModal() {
    if (!modalSkinData) return
    saving = true
    try {
      const preset = {
        id: editingPreset?.id || crypto.randomUUID(),
        name: modalName.trim() || 'Skin',
        model: modalModel,
        skin_data: modalSkinData,
        cape_data: modalCapeData || null,
        cape_id: modalCapeId || null,
        skin_url: editingPreset?.skin_url || null,
      }
      await invoke('save_skin_preset', { preset })
      await loadPresets()
      addToast(get(t)('skinManager.editPreset') + ': ' + preset.name, 'success')
      closeModal()
    } catch (e) {
      addToast(String(e), 'error')
    } finally { saving = false }
  }

  function drawModalPreview() {
    if (!modalPreviewCanvas || !modalSkinData) return
    const img = new Image()
    img.onload = () => {
      const scale = 6
      const armW = modalModel === 'slim' ? 3 : 4
      const totalW = armW * 2 + 8
      modalPreviewCanvas.width = totalW * scale
      modalPreviewCanvas.height = 32 * scale
      const ctx = modalPreviewCanvas.getContext('2d')
      ctx.imageSmoothingEnabled = false
      ctx.clearRect(0, 0, modalPreviewCanvas.width, modalPreviewCanvas.height)
      const cx = armW
      const b = (sx, sy, sw, sh, dx, dy) =>
        ctx.drawImage(img, sx, sy, sw, sh, dx * scale, dy * scale, sw * scale, sh * scale)
      b(8,8,8,8, cx,0); b(40,8,8,8, cx,0)
      b(20,20,8,12, cx,8); b(20,36,8,12, cx,8)
      b(44,20,armW,12, cx-armW,8); b(44,36,armW,12, cx-armW,8)
      b(36,52,armW,12, cx+8,8); b(52,52,armW,12, cx+8,8)
      b(4,20,4,12, cx,20); b(4,36,4,12, cx,20)
      b(20,52,4,12, cx+4,20); b(4,52,4,12, cx+4,20)
    }
    img.src = `data:image/png;base64,${modalSkinData}`
  }

  function pickSkinFile() { fileInput?.click() }

  function readFileAsBase64(file) {
    return new Promise(resolve => {
      const r = new FileReader()
      r.onload = e => resolve(e.target.result.split(',')[1])
      r.readAsDataURL(file)
    })
  }

  async function onSkinFileChange(e) {
    const file = e.target.files[0]; if (!file) return
    await loadSkinFile(file)
    e.target.value = ''
  }
</script>

{#if !$isOnline}
  <div class="offline-overlay">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="40" height="40"><line x1="1" y1="1" x2="23" y2="23"/><path d="M16.72 11.06A10.94 10.94 0 0 1 19 12.55M5 12.55a10.94 10.94 0 0 1 5.17-2.39M10.71 5.05A16 16 0 0 1 22.56 9M1.42 9a15.91 15.91 0 0 1 4.7-2.88M8.53 16.11a6 6 0 0 1 6.95 0M12 20h.01"/></svg>
    <span class="offline-overlay-title">Kein Internet</span>
    <span class="offline-overlay-sub">Skin-Verwaltung ist offline nicht verfügbar.</span>
  </div>
{:else}
<div class="page">
  <div class="page-header">
    <div>
      <h1 class="page-title">{$t('skinManager.title')}</h1>
      <p class="header-sub text-muted">{$t('skinManager.subtitle')}</p>
    </div>
    <div class="header-actions">
      {#if loadingCurrent}
        <span class="text-muted" style="font-size:12px;display:flex;align-items:center;gap:6px">
          <span class="spinner-sm"></span> {$t('skinManager.loading')}
        </span>
      {/if}
      <button class="btn btn-primary" on:click={openAddModal}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="13" height="13"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        {$t('skinManager.addPreset')}
      </button>
    </div>
  </div>

  <div class="page-body">
    <div class="skin-layout">

      <!-- Left: frameless 3D viewer -->
      <div class="viewer-panel">
        {#if activeAccount && selectedPreset}
          <div class="nametag"><span>{activeAccount.username}</span></div>
        {/if}
        <!-- Glow lives outside overflow:hidden so it bleeds below the container -->
        <div class="viewer-glow"></div>
        <div class="viewer-area" bind:this={viewerArea}>
          <canvas bind:this={viewerCanvas} class="viewer-canvas"></canvas>
          <div class="viewer-shadow" bind:this={shadowEl}></div>
          {#if !selectedPreset}
            <div class="viewer-placeholder">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="52" height="52" style="color:var(--border)"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
              <span class="text-muted" style="font-size:12px">{$t('skinManager.noSkin')}</span>
            </div>
          {/if}
          <!-- Footer card overlapping the bottom glow -->
          {#if selectedPreset}
            <div class="viewer-footer">
              {#if applying}
                <span class="applying-chip"><span class="spinner-sm"></span> {$t('skinManager.applying')}</span>
              {:else if activePresetId === selectedPreset.id}
                <span class="active-chip">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="10" height="10"><polyline points="20 6 9 17 4 12"/></svg>
                  {$t('skinManager.activeOnMojang')}
                </span>
              {:else}
                <span></span>
              {/if}
              <button class="btn btn-ghost btn-sm" on:click|stopPropagation={() => openEditModal(selectedPreset)}>
                {$t('common.edit')}
              </button>
            </div>
          {/if}
        </div>
      </div>

      <!-- Right: preset cards -->
      <div class="presets-panel">
        {#if presets.length === 0}
          <div class="empty-state">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="44" height="44" style="color:var(--border)"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
            <p class="text-muted" style="font-size:13px">{$t('skinManager.noPresets')}</p>
            <button class="btn btn-primary btn-sm" on:click={openAddModal}>{$t('skinManager.addPreset')}</button>
          </div>
        {:else}
          <div class="preset-grid">
            {#each presets as preset (preset.id)}
              <!-- svelte-ignore a11y-click-events-have-key-events -->
              <!-- svelte-ignore a11y-no-static-element-interactions -->
              <div
                class="preset-card"
                class:selected={selectedPreset?.id === preset.id}
                on:click={() => confirmDeleteId !== preset.id && selectPreset(preset)}
              >
                <!-- Delete confirmation overlay -->
                {#if confirmDeleteId === preset.id}
                  <!-- svelte-ignore a11y-click-events-have-key-events -->
                  <!-- svelte-ignore a11y-no-static-element-interactions -->
                  <div class="delete-confirm" on:click|stopPropagation>
                    <p class="delete-confirm-text">{$t('common.delete')}?</p>
                    <div class="delete-confirm-actions">
                      <button class="confirm-btn confirm-cancel" on:click={cancelDelete}>{$t('common.cancel')}</button>
                      <button class="confirm-btn confirm-delete" disabled={deleting === preset.id} on:click={() => confirmDelete(preset)}>
                        {#if deleting === preset.id}<span class="spinner-sm"></span>{/if}
                        {$t('common.delete')}
                      </button>
                    </div>
                  </div>
                {/if}

                <!-- Active indicator -->
                {#if activePresetId === preset.id}
                  <span class="active-dot">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="9" height="9"><polyline points="20 6 9 17 4 12"/></svg>
                  </span>
                {/if}

                <!-- Action buttons (appear on hover) -->
                <div class="card-actions">
                  <button class="icon-btn" title={$t('common.edit')} on:click|stopPropagation={() => openEditModal(preset)}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
                  </button>
                  <button class="icon-btn del-btn" title={$t('common.delete')} on:click|stopPropagation={() => requestDelete(preset)}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4h6v2"/></svg>
                  </button>
                </div>

                <!-- Skin upper body preview -->
                <div class="card-preview">
                  <canvas use:skinUpper={{ skinData: preset.skin_data, model: preset.model }}></canvas>
                </div>

                <!-- Name + cape -->
                <div class="card-info">
                  <span class="card-name">{preset.name}</span>
                  {#if preset.cape_id || preset.cape_data}
                    <span class="card-cape">
                      {availableCapes.find(c => c.id === preset.cape_id)?.alias || 'Cape'}
                    </span>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>
{/if}

<!-- Modal -->
{#if showModal}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="modal-backdrop" on:click={closeModal}></div>
  <div class="modal card">
    <div class="modal-header">
      <h2 class="modal-title">
        {editingPreset ? $t('skinManager.editPreset') : modalStep === 1 ? $t('skinManager.stepSkin') : $t('skinManager.stepDetails')}
      </h2>
      <button class="icon-btn" on:click={closeModal}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="15" height="15"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
      </button>
    </div>

    {#if modalStep === 1}
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <!-- svelte-ignore a11y-click-events-have-key-events -->
      <div
        class="dropzone"
        class:drag-over={dragging}
        on:dragover={onDragOver}
        on:dragleave={onDragLeave}
        on:drop={onDrop}
        on:click={pickSkinFile}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="48" height="48" style="color:var(--border)"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
        <p class="dropzone-title">{$t('skinManager.dropSkin')}</p>
        <p class="dropzone-sub text-muted">{$t('skinManager.dropClick')}</p>
      </div>

    {:else}
      <div class="modal-body">
        <div class="modal-preview-wrap">
          <canvas bind:this={modalPreviewCanvas} class="modal-canvas"></canvas>
        </div>
        <div class="modal-fields">
          <div class="field">
            <label class="field-label" for="modal-preset-name">{$t('skinManager.presetName')}</label>
            <input id="modal-preset-name" class="input" bind:value={modalName} placeholder={$t('skinManager.presetNamePlaceholder')} />
          </div>
          <div class="field">
            <!-- svelte-ignore a11y-label-has-associated-control -->
            <label class="field-label">{$t('skinManager.model')}</label>
            <div class="model-toggle">
              <button class="model-btn" class:active={modalModel === 'classic'}
                on:click={() => { modalModel = 'classic'; drawModalPreview() }}>{$t('skinManager.classic')}</button>
              <button class="model-btn" class:active={modalModel === 'slim'}
                on:click={() => { modalModel = 'slim'; drawModalPreview() }}>{$t('skinManager.slim')}</button>
            </div>
          </div>
          <div class="field">
            <button class="btn btn-ghost btn-sm" style="width:fit-content" on:click={pickSkinFile}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="12" height="12"><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
              {$t('skinManager.changeSkin')}
            </button>
          </div>
          <div class="field">
            <!-- svelte-ignore a11y-label-has-associated-control -->
            <label class="field-label">{$t('skinManager.cape')} <span class="text-muted">({$t('common.optional')})</span></label>
            {#if loadingCapes}
              <span class="text-muted" style="font-size:12px;display:flex;align-items:center;gap:6px"><span class="spinner-sm"></span> {$t('skinManager.loadingCapes')}</span>
            {:else if availableCapes.length === 0}
              <span class="text-muted" style="font-size:12px">{$t('skinManager.noCapes')}</span>
            {:else}
              <!-- svelte-ignore a11y-no-static-element-interactions -->
              <div class="cape-carousel" on:wheel|preventDefault={(e) => e.currentTarget.scrollLeft += e.deltaY}>
                <!-- svelte-ignore a11y-click-events-have-key-events -->
                <!-- svelte-ignore a11y-no-static-element-interactions -->
                <div class="cape-slot" class:selected={modalCapeId === null}
                  on:click={() => { modalCapeId = null; modalCapeData = null }}>
                  <div class="cape-face-none">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                  </div>
                  <span class="cape-name">{$t('skinManager.noCape')}</span>
                </div>
                {#each availableCapes as cape}
                  <!-- svelte-ignore a11y-click-events-have-key-events -->
                  <!-- svelte-ignore a11y-no-static-element-interactions -->
                  <div class="cape-slot" class:selected={modalCapeId === cape.id}
                    on:click={() => { modalCapeId = cape.id; modalCapeData = cape.cape_data }}>
                    <canvas use:capeFront={{ capeData: cape.cape_data }} class="cape-face-canvas"></canvas>
                    <span class="cape-name">{cape.alias}</span>
                    {#if cape.state === 'ACTIVE'}<span class="cape-dot"></span>{/if}
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        </div>
      </div>
      <div class="modal-footer">
        <button class="btn btn-ghost" on:click={closeModal}>{$t('common.cancel')}</button>
        <button class="btn btn-primary" on:click={saveModal} disabled={saving || !modalSkinData}>
          {#if saving}<span class="spinner-sm"></span>{/if}
          {$t('common.save')}
        </button>
      </div>
    {/if}
  </div>
  <input bind:this={fileInput} type="file" accept="image/png" style="display:none" on:change={onSkinFileChange} />
{/if}

<style>
  .offline-overlay { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; height: 100%; color: #fb923c; }
  .offline-overlay-title { font-size: 15px; font-weight: 700; color: #fb923c; }
  .offline-overlay-sub { font-size: 13px; color: var(--text-muted); }

  .header-sub { font-size: 13px; margin-top: 2px; }
  .header-actions { display: flex; gap: 8px; align-items: center; }

  /* ── Layout ── */
  .skin-layout {
    display: grid;
    grid-template-columns: minmax(240px, 30%) 1fr;
    gap: 24px;
    height: calc(100vh - 130px);
    min-height: 0;
  }

  /* ── Viewer panel (frameless) ── */
  .viewer-panel {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
    position: relative;  /* anchor for .viewer-glow */
  }

  /* Glow element — outside overflow:hidden so it bleeds freely below the canvas */
  .viewer-glow {
    position: absolute;
    bottom: 30px;         /* sits near the bottom of the panel */
    left: -10%;
    right: -10%;
    height: 140px;
    background: radial-gradient(ellipse at 50% 100%, rgba(var(--accent-rgb),0.18) 0%, transparent 65%);
    pointer-events: none;
    z-index: 0;
  }

  .viewer-area {
    flex: 1;
    position: relative;
    min-height: 0;
    border-radius: var(--radius);
    overflow: hidden;
    background: transparent;
    /* bottom padding so the model doesn't sit behind the footer card */
    padding-bottom: 52px;
    z-index: 1;
  }

  .viewer-canvas {
    width: 100%;
    height: 100%;
    display: block;
  }

  /* Minecraft nametag — above the 3D canvas */
  .nametag {
    display: flex;
    justify-content: center;
    flex-shrink: 0;
    padding-bottom: 8px;
  }
  .nametag span {
    font-family: 'Minecraft', monospace;
    font-size: 16px;
    color: #f0f0f0;
    background: rgba(0,0,0,0.55);
    padding: 5px 12px 7px;
    white-space: nowrap;
    /* Minecraft hard pixel shadow: offset 2px, dark gray */
    text-shadow: 2px 2px 0 #3b3b3b;
    line-height: 1;
    -webkit-font-smoothing: none;
    font-smooth: never;
  }

  /* Ground shadow ellipse — positioned at actual foot level (~32% from bottom) */
  .viewer-shadow {
    position: absolute;
    bottom: 32%;
    left: 50%;
    transform: translateX(-50%);
    width: 80px;
    height: 18px;
    background: radial-gradient(ellipse at center, rgba(0,0,0,0.45) 0%, transparent 68%);
    pointer-events: none;
    z-index: 1;
  }

  .viewer-placeholder {
    position: absolute; inset: 0;
    display: flex; flex-direction: column;
    align-items: center; justify-content: center; gap: 8px;
    pointer-events: none;
  }

  .viewer-footer {
    position: absolute;
    bottom: 10px;
    left: 10px;
    right: 10px;
    z-index: 4;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 10px;
    background: color-mix(in srgb, var(--surface) 80%, transparent);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
  }

  /* ── Preset grid ── */
  .presets-panel {
    overflow-y: auto;
    min-height: 0;
  }

  .preset-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(110px, 1fr));
    gap: 10px;
    align-content: start;
  }

  .preset-card {
    position: relative;
    border: 1.5px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    cursor: pointer;
    background: var(--surface);
    transition: all var(--transition);
    box-shadow: 0 1px 4px rgba(0,0,0,0.15);
  }
  .preset-card:hover { border-color: rgba(var(--accent-rgb),0.4); box-shadow: 0 4px 14px rgba(0,0,0,0.25); transform: translateY(-1px); }
  .preset-card.selected { border-color: var(--accent); box-shadow: 0 0 0 1px rgba(var(--accent-rgb),0.3), 0 4px 16px rgba(var(--accent-rgb),0.15); }

  /* Skin upper-body preview */
  .card-preview {
    background: var(--surface2);
    height: 120px;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    overflow: hidden;
    padding-bottom: 4px;
  }
  .card-preview canvas {
    image-rendering: pixelated;
    height: 112px;
    width: auto;
  }

  /* Card text section */
  .card-info {
    padding: 8px 9px 9px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .card-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .card-cape {
    font-size: 10px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Active indicator (green dot with checkmark, top-left) */
  .active-dot {
    position: absolute;
    top: 6px;
    left: 6px;
    z-index: 3;
    width: 18px; height: 18px;
    border-radius: 50%;
    background: var(--success);
    display: flex; align-items: center; justify-content: center;
    box-shadow: 0 0 8px rgba(52,211,153,0.5);
    color: #fff;
  }

  /* Hover action buttons (top-right) */
  .card-actions {
    position: absolute;
    top: 5px;
    right: 5px;
    z-index: 3;
    display: flex;
    gap: 3px;
    opacity: 0;
    transition: opacity var(--transition);
  }
  .preset-card:hover .card-actions { opacity: 1; }

  /* ── Status chips ── */
  .active-chip {
    display: inline-flex; align-items: center; gap: 4px;
    font-size: 11px; font-weight: 600; color: var(--success);
    padding: 3px 10px; border-radius: 100px;
    border: 1px solid rgba(52,211,153,0.3); background: rgba(52,211,153,0.08);
  }
  .applying-chip {
    display: inline-flex; align-items: center; gap: 6px;
    font-size: 11px; font-weight: 600; color: var(--accent);
    padding: 3px 10px; border-radius: 100px;
    border: 1px solid rgba(var(--accent-rgb),0.3); background: rgba(var(--accent-rgb),0.08);
  }

  /* ── Icon buttons ── */
  .icon-btn {
    width: 26px; height: 26px; border-radius: var(--radius-sm);
    display: flex; align-items: center; justify-content: center;
    color: var(--text-muted); transition: all var(--transition);
    background: rgba(0,0,0,0.45); border: none; cursor: pointer; padding: 0;
    backdrop-filter: blur(4px);
  }
  .icon-btn:hover { background: var(--surface3); color: var(--text); }
  .icon-btn.del-btn:hover { color: var(--error); background: rgba(248,113,113,0.15); }

  /* ── Empty state ── */
  .empty-state {
    display: flex; flex-direction: column; align-items: center;
    justify-content: center; gap: 12px; padding: 48px 0; text-align: center;
  }

  /* ── Drop zone ── */
  .dropzone {
    margin: 20px; border: 2px dashed var(--border);
    border-radius: var(--radius); padding: 56px 24px;
    display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px;
    cursor: pointer; transition: all var(--transition);
  }
  .dropzone:hover, .dropzone.drag-over {
    border-color: var(--accent); background: rgba(var(--accent-rgb),0.05);
  }
  .dropzone.drag-over svg { color: var(--accent) !important; }
  .dropzone-title { font-size: 15px; font-weight: 600; color: var(--text); }
  .dropzone-sub { font-size: 12px; }

  /* ── Modal ── */
  .modal-backdrop {
    position: fixed; inset: 0; background: rgba(0,0,0,0.65);
    z-index: 200; backdrop-filter: blur(3px);
  }
  .modal {
    position: fixed; top: 50%; left: 50%;
    transform: translate(-50%, -50%);
    z-index: 201; width: 500px; max-width: calc(100vw - 32px);
    display: flex; flex-direction: column;
  }
  .modal-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 16px 20px; border-bottom: 1px solid var(--border);
  }
  .modal-title { font-size: 15px; font-weight: 600; }
  .modal-body { display: flex; gap: 20px; padding: 20px; align-items: flex-start; }
  .modal-preview-wrap {
    flex-shrink: 0; width: 88px; height: 160px;
    background: radial-gradient(ellipse at center, var(--surface3) 0%, var(--bg) 100%);
    border-radius: var(--radius-sm); border: 1px solid var(--border);
    display: flex; align-items: center; justify-content: center; overflow: hidden;
  }
  .modal-canvas { image-rendering: pixelated; max-width: 100%; max-height: 100%; object-fit: contain; }
  .modal-fields { flex: 1; display: flex; flex-direction: column; gap: 14px; min-width: 0; }
  .field { display: flex; flex-direction: column; gap: 6px; }
  .field-label { font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em; color: var(--text-muted); }
  .model-toggle {
    display: flex; background: var(--surface2); border-radius: var(--radius-sm);
    padding: 3px; gap: 2px; border: 1px solid var(--border); width: fit-content;
  }
  .model-btn {
    padding: 5px 14px; border-radius: calc(var(--radius-sm) - 2px);
    font-size: 12px; font-weight: 500; color: var(--text-muted);
    background: none; border: none; cursor: pointer; transition: all var(--transition);
  }
  .model-btn.active { background: var(--accent); color: #fff; }

  /* Cape carousel */
  .cape-carousel {
    display: flex; flex-direction: row; gap: 6px;
    overflow-x: auto; overflow-y: hidden; padding-bottom: 4px;
  }
  .cape-slot {
    position: relative; flex-shrink: 0; cursor: pointer;
    border-radius: var(--radius); overflow: hidden;
    border: 2px solid transparent;
    transition: border-color var(--transition);
  }
  .cape-slot:hover { border-color: var(--text-muted); }
  .cape-slot:hover .cape-name { opacity: 1; }
  .cape-slot.selected { border-color: var(--accent); }
  .cape-face-canvas { display: block; width: 44px; height: 70px; image-rendering: pixelated; }
  .cape-face-none {
    width: 44px; height: 70px; display: flex; align-items: center;
    justify-content: center; background: var(--surface2); color: var(--text-muted);
  }
  .cape-name {
    position: absolute; bottom: 0; left: 0; right: 0;
    background: linear-gradient(transparent, rgba(0,0,0,0.75));
    color: #fff; font-size: 9px; font-weight: 600;
    text-align: center; padding: 8px 3px 4px;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    opacity: 0; transition: opacity 0.12s ease;
  }
  .cape-dot {
    position: absolute; top: 4px; right: 4px;
    width: 6px; height: 6px; border-radius: 50%;
    background: var(--success); box-shadow: 0 0 4px rgba(52,211,153,0.6);
  }

  .modal-footer {
    display: flex; align-items: center; justify-content: flex-end;
    gap: 8px; padding: 14px 20px; border-top: 1px solid var(--border);
  }

  /* ── Delete confirmation overlay ── */
  .delete-confirm {
    position: absolute;
    inset: 0;
    z-index: 10;
    background: rgba(0,0,0,0.78);
    backdrop-filter: blur(2px);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    border-radius: calc(var(--radius) - 2px);
  }
  .delete-confirm-text {
    font-size: 11px;
    font-weight: 600;
    color: #fff;
  }
  .delete-confirm-actions { display: flex; gap: 6px; }
  .confirm-btn {
    padding: 5px 10px; border-radius: var(--radius-sm);
    font-size: 11px; font-weight: 600; cursor: pointer;
    border: none; transition: all var(--transition);
  }
  .confirm-cancel {
    background: var(--surface3); color: var(--text-muted);
  }
  .confirm-cancel:hover { color: var(--text); }
  .confirm-delete {
    background: var(--error); color: #fff;
    display: flex; align-items: center; gap: 5px;
  }
  .confirm-delete:hover { background: #f87171; }
  .confirm-delete:disabled { opacity: 0.6; cursor: not-allowed; }

  .spinner-sm {
    width: 12px; height: 12px; border: 1.5px solid rgba(255,255,255,0.3);
    border-top-color: #fff; border-radius: 50%;
    animation: spin 0.7s linear infinite; flex-shrink: 0;
  }
  @keyframes spin { to { transform: rotate(360deg) } }
</style>
