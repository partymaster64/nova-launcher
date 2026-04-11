<script>
  import { onMount } from 'svelte'
  import { get } from 'svelte/store'
  import { invoke } from '@tauri-apps/api/core'
  import { config, accounts, currentPage, detailInstanceId, detailActiveTab, addToast, instanceUpdates, crashEvent } from '../../store.js'
  import Discover from './Discover.svelte'
  import LaunchOverlay from '../components/LaunchOverlay.svelte'
  import { loaderIcon, loaderColor as getLoaderColorFn, loaderLabel as getLoaderLabelFn } from '../loaderIcons.js'
  import Select from '../components/Select.svelte'
  import { t } from '../i18n.js'

  let cfg = null
  let accs = []
  let instanceId = null

  config.subscribe(v => (cfg = v))
  accounts.subscribe(v => (accs = v))
  detailInstanceId.subscribe(v => { instanceId = v })

  // Live running state + logs via polling
  let isRunning = false
  let launchingId = null
  let liveLogs = []
  let logViewerEl = null

  let details = null
  let loading = false
  let error = null
  let serverPings = {} // ip -> { loading, result, error }
  let activeTab = get(detailActiveTab) || 'content'
  detailActiveTab.set('content') // reset so next open defaults to content
  // When already on this page, react to external tab changes (e.g. "Open logs" from crash modal)
  detailActiveTab.subscribe(v => {
    if (v && v !== 'content') {
      activeTab = v
      detailActiveTab.set('content')
    }
  })

  // Content sub-tab
  let contentSubTab = 'mods'

  // Discover modal for adding content
  let showDiscoverModal = false
  let discoverCategory = 'mod'

  // Content updates — driven by global instanceUpdates store
  let updatesMap = {}   // snapshot from store
  instanceUpdates.subscribe(u => { updatesMap = u })

  let checkingUpdates = false
  let updatingItem = null   // "contentType:filename" being updated
  let updatingAll = false

  $: contentUpdates = instance ? (updatesMap[instance.id] ?? null) : null
  $: totalUpdateCount = contentUpdates?.length ?? 0

  async function checkUpdates() {
    if (!instance) return
    checkingUpdates = true
    try {
      const updates = await invoke('check_instance_updates', { instanceId: instance.id })
      instanceUpdates.update(u => ({ ...u, [instance.id]: updates }))
    } catch (e) { console.error(e) } finally { checkingUpdates = false }
  }

  async function updateSingleItem(upd) {
    const key = `${upd.content_type}:${upd.filename}`
    updatingItem = key
    try {
      await invoke('update_content', {
        instanceId: instance.id,
        contentType: upd.content_type,
        oldFilename: upd.filename,
        projectId: upd.project_id,
        newVersionId: upd.latest_version_id,
        title: upd.title,
        iconUrl: upd.icon_url || null,
      })
      instanceUpdates.update(u => ({
        ...u,
        [instance.id]: (u[instance.id] || []).filter(x => !(x.filename === upd.filename && x.content_type === upd.content_type)),
      }))
      await loadDetails(instance.id)
    } catch (e) { console.error(e) } finally { updatingItem = null }
  }

  async function updateAll() {
    if (!contentUpdates) return
    updatingAll = true
    for (const upd of [...contentUpdates]) {
      await updateSingleItem(upd)
    }
    updatingAll = false
  }

  let editMode = false
  let editName = ''
  let editRamMax = 2048
  let editJavaPath = ''
  let editCustomJava = false
  let editWidth = 854
  let editHeight = 480
  let editIcon = null       // base64 data URL preview
  let editIconSourceUrl = null // original HTTPS URL the icon came from (for Discord RPC)
  let saving = false
  // Edit modal sections
  let editSection = 'general'
  let editGroup = ''
  let editFullscreen = false
  let editCustomJvmArgs = ''
  let editEnvVars = [] // [{key, value}]
  let editPreLaunch = ''
  let editWrapper = ''
  let editPostExit = ''
  // Installation section
  let editGameVersion = ''
  let editLoader = 'vanilla'
  let editLoaderVersion = ''
  let editLoaderVersions = []
  let editLoadingLoaderVersions = false
  let editInstalling = false
  let editInstallError = null
  // System RAM for snap points
  let systemRamMb = 8192

  // Gallery state
  let galleryIdx = null
  let galleryFullSrc = null
  let galleryLoading = false
  let ssCompressing = false
  let ssCopying = false

  // Thumbnail cache: path -> data URL (undefined = not loaded, null = failed, string = ready)
  let screenshotUrls = {}
  let screenshotLoading = new Set()

  // Clear thumbnail cache only when instance changes
  let _thumbCacheId = null
  $: if (instanceId && instanceId !== _thumbCacheId) {
    screenshotUrls = {}
    screenshotLoading = new Set()
    _thumbCacheId = instanceId
  }

  $: if (activeTab === 'screenshots' && details?.screenshots) loadScreenshots()

  async function loadScreenshots() {
    if (!details?.screenshots) return
    const todo = details.screenshots.filter(ss =>
      screenshotUrls[ss.path] === undefined && !screenshotLoading.has(ss.path)
    )
    if (!todo.length) return
    for (const ss of todo) screenshotLoading.add(ss.path)
    for (let i = 0; i < todo.length; i += 3) {
      await Promise.all(todo.slice(i, i + 3).map(async ss => {
        try {
          screenshotUrls[ss.path] = await invoke('read_screenshot_thumb', { path: ss.path })
        } catch {
          screenshotUrls[ss.path] = null
        }
        screenshotLoading.delete(ss.path)
        screenshotUrls = { ...screenshotUrls }
      }))
    }
  }

  function parseScreenshotDate(filename) {
    const m = filename.match(/^(\d{4})-(\d{2})-(\d{2})_(\d{2})\.(\d{2})\.(\d{2})/)
    if (!m) return filename.replace(/\.[^.]+$/, '')
    const [, year, month, day, hour, min] = m
    const d = new Date(+year, +month - 1, +day, +hour, +min)
    return d.toLocaleDateString(cfg?.language || 'de', { day: 'numeric', month: 'short', year: 'numeric' }) + ', ' + `${hour}:${min}`
  }

  async function openGallery(idx) {
    galleryIdx = idx
    galleryFullSrc = null
    galleryLoading = true
    const ss = details.screenshots[idx]
    try {
      galleryFullSrc = await invoke('read_screenshot', { path: ss.path })
    } catch {
      galleryFullSrc = screenshotUrls[ss.path] ?? null
    }
    galleryLoading = false
  }

  function closeGallery() { galleryIdx = null; galleryFullSrc = null }

  function prevGallery() {
    if (galleryIdx === null || !details?.screenshots?.length) return
    openGallery((galleryIdx - 1 + details.screenshots.length) % details.screenshots.length)
  }

  function nextGallery() {
    if (galleryIdx === null || !details?.screenshots?.length) return
    openGallery((galleryIdx + 1) % details.screenshots.length)
  }

  function handleGalleryKey(e) {
    if (galleryIdx === null) return
    if (e.key === 'Escape') closeGallery()
    else if (e.key === 'ArrowLeft') prevGallery()
    else if (e.key === 'ArrowRight') nextGallery()
  }

  async function deleteGalleryItem() {
    if (galleryIdx === null) return
    const ss = details.screenshots[galleryIdx]
    try {
      await invoke('delete_screenshot', { path: ss.path })
      delete screenshotUrls[ss.path]
      screenshotUrls = { ...screenshotUrls }
      details.screenshots = details.screenshots.filter((_, i) => i !== galleryIdx)
      details = { ...details }
      if (details.screenshots.length === 0) closeGallery()
      else openGallery(Math.min(galleryIdx, details.screenshots.length - 1))
    } catch (e) { addToast(String(e), 'error') }
  }

  async function copyGalleryItem() {
    const src = galleryFullSrc || (galleryIdx !== null ? screenshotUrls[details.screenshots[galleryIdx]?.path] : null)
    if (!src) return
    ssCopying = true
    try {
      const res = await fetch(src)
      const blob = await res.blob()
      await navigator.clipboard.write([new ClipboardItem({ [blob.type]: blob })])
      addToast(get(t)('instanceDetail.copiedToClipboard'), 'success')
    } catch (e) { addToast(String(e), 'error') }
    ssCopying = false
  }

  async function compressGalleryItem() {
    if (galleryIdx === null) return
    const ss = details.screenshots[galleryIdx]
    ssCompressing = true
    try {
      const newPath = await invoke('compress_screenshot', { path: ss.path })
      delete screenshotUrls[ss.path]
      screenshotUrls = { ...screenshotUrls }
      await loadDetails(instance.id)
      const newIdx = details.screenshots.findIndex(s => s.path === newPath)
      if (newIdx >= 0) await openGallery(newIdx)
      else closeGallery()
      addToast(get(t)('instanceDetail.compressed'), 'success')
    } catch (e) { addToast(String(e), 'error') }
    ssCompressing = false
  }

  onMount(() => {
    const id = get(detailInstanceId)
    if (id) {
      instanceId = id
      loadDetails(id).then(() => { if (activeTab === 'servers') pingAllServers() })
      // Load cached updates immediately, then trigger fresh check in background
      invoke('get_instance_updates', { instanceId: id })
        .then(cached => { if (cached != null) instanceUpdates.update(u => ({ ...u, [id]: cached })) })
        .catch(() => {})
      invoke('check_instance_updates', { instanceId: id })
        .then(updates => instanceUpdates.update(u => ({ ...u, [id]: updates })))
        .catch(() => {})
    }

    window.addEventListener('keydown', handleGalleryKey)

    const pollIv = setInterval(async () => {
      if (!instanceId) return
      try {
        const running = await invoke('get_running_instances')
        isRunning = running.includes(instanceId)
        if (isRunning && activeTab === 'logs') {
          liveLogs = await invoke('get_instance_logs', { instanceId })
        }

        if (launchingId && running.includes(launchingId)) launchingId = null
      } catch (_) {}
    }, 750)

    return () => { clearInterval(pollIv); window.removeEventListener('keydown', handleGalleryKey) }
  })

  $: instance = details?.instance || cfg?.instances?.find(i => i.id === instanceId)
  $: activeAccount = accs.find(a => a.uuid === cfg?.active_account_uuid)
  // Clear LaunchOverlay if crash modal appears
  $: if ($crashEvent) launchingId = null

  // Load logs immediately when switching to the logs tab while the game is running
  $: if (activeTab === 'logs' && isRunning && instanceId) {
    invoke('get_instance_logs', { instanceId }).then(logs => { liveLogs = logs }).catch(() => {})
  }

  let iconSrc = null
  $: if (instance?.icon_path) {
    invoke('read_icon', { path: instance.icon_path })
      .then(url => { iconSrc = url })
      .catch(() => { iconSrc = null })
  } else {
    iconSrc = null
  }

  async function loadDetails(id) {
    loading = true
    error = null
    try {
      details = await invoke('get_instance_details', { instanceId: id })
      syncEdit()
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }

  async function pingAllServers() {
    if (!details?.servers?.length) return
    for (const server of details.servers) {
      serverPings[server.ip] = { loading: true, result: null, error: null }
      serverPings = serverPings
      invoke('ping_server', { address: server.ip })
        .then(result => {
          serverPings[server.ip] = { loading: false, result, error: null }
          serverPings = serverPings
        })
        .catch(err => {
          serverPings[server.ip] = { loading: false, result: null, error: String(err) }
          serverPings = serverPings
        })
    }
  }

  function syncEdit() {
    if (!details?.instance) return
    const i = details.instance
    editName = i.name
    editRamMax = i.ram_max_mb || 2048
    editJavaPath = i.java_path || ''
    editCustomJava = !!(i.java_path)
    editWidth = i.game_width || 854
    editHeight = i.game_height || 480
    editIcon = null
    editIconSourceUrl = null
    editSection = 'general'
    editGroup = i.group || ''
    editFullscreen = i.fullscreen || false
    editCustomJvmArgs = i.custom_jvm_args || ''
    editEnvVars = (i.env_vars || []).map(v => ({ key: v[0] || '', value: v[1] || '' }))
    editPreLaunch = i.pre_launch_hook || ''
    editWrapper = i.wrapper_command || ''
    editPostExit = i.post_exit_hook || ''
    editGameVersion = i.version || ''
    editLoader = i.loader || 'vanilla'
    editLoaderVersion = i.loader_version || ''
    editLoaderVersions = []
    editInstallError = null
    // Fetch system RAM
    invoke('get_system_ram_mb').then(mb => { systemRamMb = mb }).catch(() => {})
  }

  async function saveInstance() {
    if (!details?.instance) return
    saving = true
    try {
      let latestIconPath = details.instance.icon_path ?? null

      // Save icon first if changed
      if (editIcon && editIcon.startsWith('data:')) {
        const [header, data] = editIcon.split(',')
        const ext = header.match(/image\/(\w+)/)?.[1] || 'png'
        const newCfg = await invoke('save_instance_icon', {
          instanceId: details.instance.id,
          base64Data: data,
          extension: ext,
          sourceUrl: editIconSourceUrl || null,
        })
        config.set(newCfg)
        // Grab the new icon_path from the returned config
        latestIconPath = newCfg.instances?.find(i => i.id === details.instance.id)?.icon_path ?? latestIconPath
      }

      const updated = {
        ...details.instance,
        name: editName,
        ram_min_mb: 512,
        ram_max_mb: editRamMax,
        java_path: (editCustomJava && editJavaPath) ? editJavaPath : null,
        game_width: editWidth,
        game_height: editHeight,
        icon_path: latestIconPath,
        group: editGroup || null,
        fullscreen: editFullscreen,
        custom_jvm_args: editCustomJvmArgs || null,
        env_vars: editEnvVars.filter(e => e.key.trim()).map(e => [e.key, e.value]) || null,
        pre_launch_hook: editPreLaunch || null,
        wrapper_command: editWrapper || null,
        post_exit_hook: editPostExit || null,
      }
      const newCfg = await invoke('update_instance', { instance: updated })
      config.set(newCfg)
      details = { ...details, instance: updated }
      editMode = false
      editIcon = null
    } catch (e) {
      console.error(e)
    } finally {
      saving = false
    }
  }

  // RAM snap points: capped at system RAM
  function ramSnapPoints(maxSysMb) {
    const all = [512, 1024, 2048, 4096, 6144, 8192, 12288, 16384, 24576, 32768]
    // Round up to nearest tier so e.g. 31900 MB (32 GB system) still shows the 32 GB option
    const tiers = [2048, 4096, 8192, 16384, 32768, 65536]
    const rounded = tiers.find(t => t >= maxSysMb) ?? maxSysMb
    return all.filter(v => v <= Math.max(rounded, 2048))
  }

  function nearestSnapIdx(value, points) {
    let best = 0
    let bestDiff = Math.abs(points[0] - value)
    for (let i = 1; i < points.length; i++) {
      const diff = Math.abs(points[i] - value)
      if (diff < bestDiff) { bestDiff = diff; best = i }
    }
    return best
  }

  $: ramSnaps = ramSnapPoints(systemRamMb)
  $: ramMaxIdx = nearestSnapIdx(editRamMax, ramSnaps)
  function onRamMaxSlider(e) {
    editRamMax = ramSnaps[parseInt(e.target.value)]
  }

  async function fetchEditLoaderVersions() {
    if (editLoader === 'vanilla') { editLoaderVersions = []; return }
    editLoadingLoaderVersions = true
    try {
      editLoaderVersions = await invoke('get_loader_versions', {
        loader: editLoader,
        mcVersion: editGameVersion || instance?.version || ''
      })
    } catch (e) { editLoaderVersions = [] }
    finally { editLoadingLoaderVersions = false }
  }

  $: if (editMode && editSection === 'installation') {
    if (editLoader !== 'vanilla') fetchEditLoaderVersions()
  }

  // Compute which Java version is required for this MC version
  function requiredJavaMajor(mcVersion) {
    if (!mcVersion) return 21
    const parts = mcVersion.split('.').map(Number).filter(n => !isNaN(n))
    if (parts[0] >= 26) return 21 // new MC versioning scheme (26.x+)
    if (parts[0] === 1) {
      const minor = parts[1] ?? 0
      const patch = parts[2] ?? 0
      if (minor >= 21 || (minor === 20 && patch >= 5)) return 21
      if (minor >= 17) return 17
      return 8
    }
    return 21
  }

  // Returns { path, label } for the automatically selected Java
  $: autoJavaInfo = (() => {
    const mcVer = editGameVersion || details?.instance?.version || ''
    const major = requiredJavaMajor(mcVer)
    const javaPaths = cfg?.java_paths ?? {}
    const localPath = javaPaths[String(major)]
    if (localPath) return { path: localPath, label: `Java ${major} (Lokale Installation)` }
    // fall back through newer LTS versions if available
    for (const v of [25, 21, 17, 8]) {
      if (v >= major && javaPaths[String(v)]) {
        return { path: javaPaths[String(v)], label: `Java ${v} (Lokale Installation)` }
      }
    }
    const globalPath = cfg?.global_java_path
    if (globalPath) return { path: globalPath, label: `Globaler Java-Pfad (${globalPath})` }
    return { path: '', label: `Java ${major} (aus PATH)` }
  })()

  async function applyInstallChanges() {
    if (!details?.instance) return
    editInstalling = true
    editInstallError = null
    try {
      const updated = {
        ...details.instance,
        version: editGameVersion || details.instance.version,
        loader: editLoader,
        loader_version: editLoaderVersion || null,
      }
      const newCfg = await invoke('update_instance', { instance: updated })
      config.set(newCfg)
      details = { ...details, instance: updated }
      addToast(get(t)('instanceDetail.installSaved'))
    } catch (e) {
      editInstallError = String(e)
    } finally {
      editInstalling = false
    }
  }

  async function repairInstance() {
    if (!details?.instance) return
    editInstalling = true
    editInstallError = null
    try {
      await invoke('prepare_instance', { instanceId: details.instance.id })
      addToast(get(t)('instanceDetail.repairStarted'))
    } catch (e) {
      editInstallError = String(e)
    } finally {
      editInstalling = false
    }
  }

  async function duplicateInstance() {
    if (!details?.instance) return
    try {
      const newCfg = await invoke('duplicate_instance', { instanceId: details.instance.id })
      config.set(newCfg)
      addToast(get(t)('instanceDetail.duplicated'))
      editMode = false
    } catch (e) {
      addToast(get(t)('instanceDetail.duplicateError', { error: String(e) }))
    }
  }

  let showDeleteModal = false
  let showContextMenu = false

  function toggleContextMenu(e) {
    e.stopPropagation()
    showContextMenu = !showContextMenu
  }

  function closeContextMenu() {
    showContextMenu = false
  }

  function promptDelete() {
    if (!instance) return
    showDeleteModal = true
  }

  async function confirmDelete() {
    showDeleteModal = false
    try {
      const newCfg = await invoke('delete_instance', { id: instance.id })
      config.set(newCfg)
      currentPage.set('instances')
    } catch (e) {
      console.error(e)
    }
  }

  async function openFolder() {
    if (!instance) return
    try { await invoke('open_instance_folder', { instanceId: instance.id }) } catch (e) { console.error(e) }
  }

  async function launchInstance() {
    if (!instance || !activeAccount) return
    launchingId = instance.id
    try {
      await invoke('launch_instance', { instanceId: instance.id })
    } catch (e) {
      console.error('Launch error:', e)
      launchingId = null
    }
  }

  async function launchWithQuickplay(targetType, target) {
    if (!instance || !activeAccount) return
    launchingId = instance.id
    try {
      await invoke('launch_with_quickplay', { instanceId: instance.id, targetType, target })
    } catch (e) {
      console.error('Launch error:', e)
      launchingId = null
    }
  }

  async function stopInstance() {
    if (!instance) return
    try { await invoke('kill_instance', { instanceId: instance.id }) } catch (e) { console.error(e) }
  }

  async function deleteMod(filename) {
    if (!instance) return
    if (!confirm(get(t)('instanceDetail.confirmDeleteFile', { filename }))) return
    try {
      await invoke('delete_mod', { filename, instanceId: instance.id })
      await loadDetails(instance.id)
    } catch (e) { console.error(e) }
  }

  async function deleteContentFile(sectionId, filename) {
    if (!instance) return
    if (!confirm(get(t)('instanceDetail.confirmDeleteFile', { filename }))) return
    try {
      await invoke('delete_content', { filename, instanceId: instance.id, contentType: sectionId })
      await loadDetails(instance.id)
    } catch (e) { console.error(e) }
  }

  function openDiscoverModal(category = 'mod') {
    discoverCategory = category
    showDiscoverModal = true
  }

  function closeDiscoverModal() {
    showDiscoverModal = false
    if (instance) loadDetails(instance.id)
  }


  function handleIconFile(event) {
    const file = event.target.files?.[0]
    if (!file) return
    const reader = new FileReader()
    reader.onload = (e) => { editIcon = e.target.result; editIconSourceUrl = null }
    reader.readAsDataURL(file)
  }

  function formatBytes(bytes) {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }

  function formatRam(mb) {
    if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`
    return `${mb} MB`
  }

  function getLoaderColor(loader) { return getLoaderColorFn(loader) }
  function getLoaderName(loader) { return getLoaderLabelFn(loader) }

  function formatPlaytime(secs) {
    if (!secs) return null
    const h = Math.floor(secs / 3600)
    const m = Math.floor((secs % 3600) / 60)
    if (h >= 1) return m > 0 ? `${h} Std ${m} Min` : `${h} Std`
    if (m >= 1) return `${m} Min`
    return '< 1 Min'
  }

  function formatLastPlayed(ms) {
    if (!ms) return null
    return new Date(ms).toLocaleDateString('de-DE', { day: '2-digit', month: '2-digit', year: 'numeric' })
  }

  // ── Mod Toggle ────────────────────────────────────────────────────────
  let togglingMod = null  // filename being toggled

  async function toggleMod(filename) {
    if (!instance || togglingMod) return
    togglingMod = filename
    try {
      await invoke('toggle_mod', { instanceId: instance.id, filename })
      // Patch the mod list in-place so the scroll position is preserved
      const wasEnabled = filename.endsWith('.jar') && !filename.endsWith('.jar.disabled')
      const newFilename = wasEnabled ? filename + '.disabled' : filename.slice(0, -'.disabled'.length)
      details = {
        ...details,
        mods: details.mods.map(m =>
          m.filename === filename
            ? { ...m, filename: newFilename, enabled: !m.enabled }
            : m
        )
      }
    } catch (e) { console.error(e) }
    finally { togglingMod = null }
  }

  // Auto-scroll log viewer to bottom on update
  function autoScroll(node) {
    const scroll = () => { node.scrollTop = node.scrollHeight }
    scroll()
    const obs = new MutationObserver(scroll)
    obs.observe(node, { childList: true })
    return { destroy() { obs.disconnect() } }
  }

  // Parse log lines for coloring — handles Minecraft's [Thread/LEVEL] format
  function parseLogLine(line) {
    const lower = line.toLowerCase()
    if (lower.includes('/fatal]') || lower.includes('/error]') || lower.includes('[error]') || lower.includes('exception in thread') || lower.includes('caused by:')) return 'error'
    if (lower.includes('/warn]')  || lower.includes('[warn]'))  return 'warn'
    if (lower.includes('/info]')  || lower.includes('[info]'))  return 'info'
    if (lower.includes('/debug]') || lower.includes('[debug]')) return 'debug'
    if (lower.includes('exception') || lower.includes('error:') || lower.includes('fatal')) return 'error'
    return 'default'
  }

  // Content sections config — mods and shaderpacks hidden for vanilla instances
  const ALL_CONTENT_SECTIONS = [
    { id: 'mods',         labelKey: 'instanceDetail.sectionMods',         category: 'mod',          icon: 'M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z' },
    { id: 'resourcepacks',labelKey: 'instanceDetail.sectionResourcePacks', category: 'resourcepack', icon: 'M3 3h18v18H3z M9 9h6v6H9z' },
    { id: 'shaderpacks',  labelKey: 'instanceDetail.sectionShaderPacks',   category: 'shader',       icon: 'M12 7a5 5 0 1 0 0 10A5 5 0 0 0 12 7z M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42' },
    { id: 'datapacks',    labelKey: 'instanceDetail.sectionDataPacks',     category: 'datapack',     icon: 'M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z M14 2v6h6' },
  ]
  $: isVanilla = !instance?.loader || instance.loader === 'vanilla'
  $: contentSections = isVanilla
    ? ALL_CONTENT_SECTIONS.filter(s => s.id === 'resourcepacks' || s.id === 'datapacks')
    : ALL_CONTENT_SECTIONS
  $: if (isVanilla && (contentSubTab === 'mods' || contentSubTab === 'shaderpacks')) contentSubTab = 'resourcepacks'
</script>

<div class="page">
  <!-- Header -->
  <div class="page-header">
    <div class="header-left">
      <button class="back-btn" on:click={() => currentPage.set('instances')}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16"><polyline points="15 18 9 12 15 6"/></svg>
      </button>
      {#if instance}
        <div class="instance-icon-wrap">
          {#if iconSrc}
            <img src={iconSrc} alt="" class="instance-icon-img" />
          {:else}
            <div class="instance-icon-placeholder" style="background: linear-gradient(135deg, {getLoaderColor(instance.loader)}33, var(--surface3)); color: {getLoaderColor(instance.loader)}">
              {instance.name[0]?.toUpperCase() || '?'}
            </div>
          {/if}
        </div>
        <div class="instance-title-block">
          <h1 class="page-title">{instance.name}</h1>
          <div class="instance-meta">
            <span class="meta-badge" style="color:{getLoaderColor(instance.loader)};background:{getLoaderColor(instance.loader)}18;border-color:{getLoaderColor(instance.loader)}33">
              {#if loaderIcon(instance.loader)}<span class="loader-icon">{@html loaderIcon(instance.loader)}</span>{/if}
              {getLoaderName(instance.loader)}
            </span>
            <span class="meta-badge">MC {instance.version || '?'}</span>
            {#if instance.total_play_secs > 0}
              <span class="meta-badge" title={$t('instanceDetail.totalPlaytime')}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="10" height="10"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
                {formatPlaytime(instance.total_play_secs)}
              </span>
            {/if}
            {#if totalUpdateCount > 0}
              <span class="meta-badge upd-chip" title="{totalUpdateCount} Mod-Update{totalUpdateCount !== 1 ? 's' : ''} verfügbar">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="10" height="10"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 .49-3.5"/></svg>
                {totalUpdateCount}
              </span>
            {/if}
          </div>
        </div>
      {/if}
    </div>
    <div class="header-actions">
      {#if isRunning}
        <span class="running-pill"><span class="running-dot"></span>{$t('instanceDetail.running')}</span>
        <button class="btn btn-danger" on:click={stopInstance}>
          <svg viewBox="0 0 24 24" fill="currentColor" width="12" height="12"><rect x="3" y="3" width="18" height="18" rx="2"/></svg>
          {$t('common.stop')}
        </button>
      {:else if instance?.version && activeAccount}
        <button class="btn btn-primary" on:click={launchInstance}>
          <svg viewBox="0 0 24 24" fill="currentColor" width="13" height="13"><polygon points="5 3 19 12 5 21 5 3"/></svg>
          {$t('common.play')}
        </button>
      {/if}
      <!-- svelte-ignore a11y-click-events-have-key-events -->
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div class="ctx-wrap" on:click|stopPropagation={() => {}}>
        <button class="icon-btn ctx-trigger" on:click={toggleContextMenu} title={$t('common.more')}>
          <svg viewBox="0 0 24 24" fill="currentColor" width="15" height="15">
            <circle cx="5"  cy="12" r="2"/><circle cx="12" cy="12" r="2"/><circle cx="19" cy="12" r="2"/>
          </svg>
        </button>
        {#if showContextMenu}
          <!-- svelte-ignore a11y-click-events-have-key-events -->
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div class="ctx-backdrop" on:click={closeContextMenu}></div>
          <div class="ctx-menu">
            <button class="ctx-item" on:click={() => { closeContextMenu(); openFolder() }}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
              {$t('common.open')}
            </button>
            <button class="ctx-item" on:click={() => { closeContextMenu(); editMode = true; syncEdit() }}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
              {$t('common.edit')}
            </button>
            <button class="ctx-item" disabled={checkingUpdates} on:click={() => { closeContextMenu(); checkUpdates() }}>
              {#if checkingUpdates}
                <div class="upd-spinner" style="width:13px;height:13px"></div>
              {:else}
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 .49-3.5"/></svg>
              {/if}
              {$t('instanceDetail.checkUpdates')}
            </button>
            <div class="ctx-divider"></div>
            <button class="ctx-item ctx-item-danger" on:click={() => { closeContextMenu(); promptDelete() }}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6m4-6v6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/></svg>
              {$t('common.delete')}
            </button>
          </div>
        {/if}
      </div>
    </div>
  </div>

  <!-- Tabs -->
  <div class="tabs">
    {#each [['content', `${$t('instanceDetail.tabContent')} (${details?.total_mods || 0})`], ['worlds', `${$t('instanceDetail.tabWorlds')} (${details?.total_worlds || 0})`], ['servers', `${$t('instanceDetail.tabServers')} (${details?.servers?.length || 0})`], ['screenshots', `${$t('instanceDetail.tabScreenshots')} (${details?.screenshots?.length || 0})`], ['logs', $t('instanceDetail.tabLogs')]] as [id, label]}
      <button class="tab" class:active={activeTab === id} on:click={() => { activeTab = id; if ((id === 'worlds' || id === 'servers') && instanceId) loadDetails(instanceId).then(() => { if (id === 'servers') pingAllServers() }) }}>
        {label}
        {#if id === 'content' && totalUpdateCount > 0}
          <span class="tab-update-badge">{totalUpdateCount}</span>
        {/if}
      </button>
    {/each}
  </div>

  <div class="page-body">
    {#if loading}
      <div class="loading-state text-muted">{$t('instanceDetail.loadingDetails')}</div>
    {:else if error}
      <div class="error-state text-error">{error}</div>
    {:else if details}

      <!-- Content tab -->
      {#if activeTab === 'content'}
        <div class="content-tab">
          <!-- Section picker (pill chips) -->
          <div class="content-section-bar">
            <div class="content-chip-group">
              {#each contentSections as section}
                {@const count = section.id === 'mods' ? details.mods.length
                              : section.id === 'resourcepacks' ? (details.resourcepacks?.length ?? 0)
                              : section.id === 'shaderpacks' ? (details.shaderpacks?.length ?? 0)
                              : section.id === 'datapacks' ? (details.datapacks?.length ?? 0)
                              : 0}
                {@const updCount = contentUpdates?.filter(u => u.content_type === section.category).length ?? 0}
                <button
                  class="content-chip"
                  class:active={contentSubTab === section.id}
                  on:click={() => contentSubTab = section.id}
                >
                  {$t(section.labelKey)}
                  {#if count > 0}<span class="chip-cnt">{count}</span>{/if}
                  {#if updCount > 0}<span class="chip-upd-cnt">{updCount}</span>{/if}
                </button>
              {/each}
            </div>
          </div>

          <!-- Active section -->
          {#each contentSections as section}
            {#if contentSubTab === section.id}
              {@const sectionCount = section.id === 'mods' ? details.mods.length
                                   : section.id === 'resourcepacks' ? (details.resourcepacks?.length ?? 0)
                                   : section.id === 'shaderpacks' ? (details.shaderpacks?.length ?? 0)
                                   : section.id === 'datapacks' ? (details.datapacks?.length ?? 0)
                                   : 0}
              {@const sectionUpdates = contentUpdates?.filter(u => u.content_type === section.category) ?? []}
              <div class="section-content">
                <div class="section-header">
                  <div class="section-title-block">
                    <span class="section-subtitle">{$t(section.labelKey)}</span>
                    {#if sectionCount > 0}<span class="section-subtitle-count">{sectionCount}</span>{/if}
                  </div>
                  <div style="display:flex;gap:6px;align-items:center">
                    <button class="btn btn-primary btn-sm" on:click={() => openDiscoverModal(section.category)}>
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="12" height="12"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
                      {$t('instanceDetail.installContent')}
                    </button>
                  </div>
                </div>

                {#if sectionUpdates.length > 0}
                  <div class="update-hint">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="11" height="11"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 .49-3.5"/></svg>
                    <span>{sectionUpdates.length} Update{sectionUpdates.length !== 1 ? 's' : ''} verfügbar</span>
                    <button class="btn btn-ghost btn-sm" disabled={updatingAll} on:click={updateAll}>
                      Alle aktualisieren
                    </button>
                  </div>
                {/if}

                {#if section.id === 'mods'}
                  {#if details.mods.length === 0}
                    <div class="empty-tab">
                      <div class="empty-icon-circle">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="28" height="28"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/></svg>
                      </div>
                      <p class="text-muted">{$t('instanceDetail.noMods')}</p>
                      <button class="btn btn-ghost btn-sm" on:click={() => openDiscoverModal(section.category)}>
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="12" height="12"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
                        {$t('instanceDetail.installContent')}
                      </button>
                    </div>
                  {:else}
                    <div class="mod-list">
                      {#each details.mods as mod}
                        {@const upd = contentUpdates?.find(u => u.filename === (mod.enabled ? mod.filename : mod.filename.replace(/\.disabled$/, '')) && u.content_type === 'mod')}
                        {@const itemKey = `mod:${mod.filename}`}
                        <div class="mod-row card" class:has-update={!!upd} class:mod-disabled={!mod.enabled}>
                          <div class="mod-icon-wrap">
                            {#if mod.icon_url}
                              <img src={mod.icon_url} alt="" class="mod-icon-img" on:error={e => e.target.style.display='none'} />
                            {:else}
                              <div class="mod-icon-placeholder">
                                <svg viewBox="0 0 24 24" fill="currentColor" width="22" height="22" opacity=".35"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/></svg>
                              </div>
                            {/if}
                          </div>
                          <div class="mod-info">
                            <span class="mod-name">{mod.title || (mod.enabled ? mod.filename : mod.filename.replace(/\.disabled$/, ''))}</span>
                            <span class="mod-filename text-muted">{mod.title ? (mod.enabled ? mod.filename : mod.filename.replace(/\.disabled$/, '')) : ''}</span>
                            <span class="mod-size text-muted">{formatBytes(mod.size_bytes)}</span>
                          </div>
                          <!-- Toggle enabled/disabled -->
                          <button
                            class="mod-toggle-btn"
                            class:mod-toggle-on={mod.enabled}
                            disabled={togglingMod === mod.filename}
                            on:click={() => toggleMod(mod.filename)}
                            title={mod.enabled ? $t('instanceDetail.modDisable') : $t('instanceDetail.modEnable')}
                          >
                            <span class="mod-toggle-thumb"></span>
                          </button>
                          {#if upd && mod.enabled}
                            <button
                              class="btn btn-sm update-btn"
                              disabled={updatingItem === itemKey || updatingAll}
                              on:click={() => updateSingleItem(upd)}
                              title="Update auf {upd.latest_version_number}"
                            >
                              {#if updatingItem === itemKey}
                                <div class="upd-spinner"></div>
                              {:else}
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 .49-3.5"/></svg>
                              {/if}
                              {upd.latest_version_number}
                            </button>
                          {/if}
                          <button class="icon-btn text-muted" on:click={() => deleteMod(mod.filename)} title={$t('common.delete')}>
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6m4-6v6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/></svg>
                          </button>
                        </div>
                      {/each}
                    </div>
                  {/if}
                {:else}
                  {@const items = section.id === 'resourcepacks' ? details.resourcepacks
                               : section.id === 'shaderpacks'   ? details.shaderpacks
                               : section.id === 'datapacks'     ? details.datapacks
                               : []}
                  {#if items.length === 0}
                    <div class="empty-tab">
                      <div class="empty-icon-circle">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="28" height="28"><path d="{section.icon}"/></svg>
                      </div>
                      <p class="text-muted">{$t('instanceDetail.noSection', { section: $t(section.labelKey) })}</p>
                      <button class="btn btn-ghost btn-sm" on:click={() => openDiscoverModal(section.category)}>
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="12" height="12"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
                        {$t('instanceDetail.installContent')}
                      </button>
                    </div>
                  {:else}
                    <div class="mod-list">
                      {#each items as item}
                        {@const upd = contentUpdates?.find(u => u.filename === item.filename && u.content_type === section.category)}
                        {@const itemKey = `${section.category}:${item.filename}`}
                        <div class="mod-row card" class:has-update={!!upd}>
                          <div class="mod-icon-wrap">
                            {#if item.icon_url}
                              <img src={item.icon_url} alt="" class="mod-icon-img" on:error={e => e.target.style.display='none'} />
                            {:else}
                              <div class="mod-icon-placeholder">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="22" height="22" opacity=".4"><path d="{section.icon}"/></svg>
                              </div>
                            {/if}
                          </div>
                          <div class="mod-info">
                            <span class="mod-name">{item.title || item.filename}</span>
                            {#if item.title}<span class="mod-filename text-muted">{item.filename}</span>{/if}
                            <span class="mod-size text-muted">{formatBytes(item.size_bytes)}</span>
                          </div>
                          {#if upd}
                            <button
                              class="btn btn-sm update-btn"
                              disabled={updatingItem === itemKey || updatingAll}
                              on:click={() => updateSingleItem(upd)}
                              title="Update auf {upd.latest_version_number}"
                            >
                              {#if updatingItem === itemKey}
                                <div class="upd-spinner"></div>
                              {:else}
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 .49-3.5"/></svg>
                              {/if}
                              {upd.latest_version_number}
                            </button>
                          {/if}
                          <button class="icon-btn text-muted" on:click={() => deleteContentFile(section.id, item.filename)} title={$t('common.delete')}>
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6m4-6v6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/></svg>
                          </button>
                        </div>
                      {/each}
                    </div>
                  {/if}
                {/if}
              </div>
            {/if}
          {/each}
        </div>

      <!-- Worlds tab -->
      {:else if activeTab === 'worlds'}
        {#if details.worlds.length === 0}
          <div class="empty-tab">
            <div class="empty-icon-circle">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="28" height="28"><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>
            </div>
            <p class="text-muted">{$t('instanceDetail.noWorlds')}</p>
          </div>
        {:else}
          <div class="worlds-list">
            {#each details.worlds as world}
              <div class="world-row card">
                <div class="world-icon">
                  {#if world.icon}
                    <img src="data:image/png;base64,{world.icon}" alt="" class="world-icon-img" />
                  {:else}
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="32" height="32"><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>
                  {/if}
                </div>
                <div class="world-info">
                  <span class="world-name">{world.display_name || world.name}</span>
                  {#if world.display_name && world.display_name !== world.name}
                    <span class="world-date text-muted">{world.name}</span>
                  {/if}
                  {#if world.last_played_ms}
                    <span class="world-date text-muted">{$t('instanceDetail.lastPlayed', { date: formatLastPlayed(world.last_played_ms) })}</span>
                  {/if}
                </div>
                <button class="btn btn-ghost btn-sm" on:click={() => launchWithQuickplay('world', world.name)} disabled={!activeAccount || !instance?.version}>
                  <svg viewBox="0 0 24 24" fill="currentColor" width="11" height="11"><polygon points="5 3 19 12 5 21 5 3"/></svg>
                  {$t('common.play')}
                </button>
              </div>
            {/each}
          </div>
        {/if}

      <!-- Servers tab -->
      {:else if activeTab === 'servers'}
        {#if !details.servers || details.servers.length === 0}
          <div class="empty-tab">
            <div class="empty-icon-circle">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="28" height="28"><rect x="2" y="3" width="20" height="4" rx="1"/><rect x="2" y="10" width="20" height="4" rx="1"/><rect x="2" y="17" width="20" height="4" rx="1"/></svg>
            </div>
            <p class="text-muted">{$t('instanceDetail.noServers')}</p>
          </div>
        {:else}
          <div class="worlds-list">
            {#each details.servers as server}
              {@const ping = serverPings[server.ip]}
              {@const icon = ping?.result?.icon || server.icon}
              <div class="server-row card">
                <!-- col 1: icon -->
                <div class="world-icon">
                  {#if icon}
                    <img src="data:image/png;base64,{icon}" alt="" class="world-icon-img" />
                  {:else}
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="32" height="32"><rect x="2" y="3" width="20" height="4" rx="1"/><rect x="2" y="10" width="20" height="4" rx="1"/><rect x="2" y="17" width="20" height="4" rx="1"/><circle cx="18" cy="5" r="1" fill="currentColor"/><circle cx="18" cy="12" r="1" fill="currentColor"/><circle cx="18" cy="19" r="1" fill="currentColor"/></svg>
                  {/if}
                </div>
                <!-- col 2: name + ip -->
                <div class="server-name-col">
                  <span class="world-name">{server.name || server.ip}</span>
                  <span class="world-date text-muted">{server.ip}</span>
                </div>
                <!-- col 3: MOTD centered, Minecraft font -->
                <div class="server-motd-col">
                  {#if ping?.result?.motd_html}
                    <span class="server-motd">{@html ping.result.motd_html}</span>
                  {:else if ping?.loading}
                    <span class="text-muted" style="font-size:11px">…</span>
                  {/if}
                </div>
                <!-- col 4: players + play button -->
                <div class="server-right">
                  {#if ping?.result?.online}
                    <span class="server-players">
                      <svg viewBox="0 0 24 24" fill="var(--accent)" width="13" height="13"><path d="M12 12c2.7 0 4.8-2.1 4.8-4.8S14.7 2.4 12 2.4 7.2 4.5 7.2 7.2 9.3 12 12 12zm0 2.4c-3.2 0-9.6 1.6-9.6 4.8v2.4h19.2v-2.4c0-3.2-6.4-4.8-9.6-4.8z"/></svg>
                      {ping.result.players_online}
                    </span>
                  {:else if ping?.result && !ping.result.online}
                    <span class="server-offline">{$t('instanceDetail.offline')}</span>
                  {/if}
                  <button class="btn btn-ghost btn-sm" on:click={() => launchWithQuickplay('server', server.ip)} disabled={!activeAccount || !instance?.version}>
                    <svg viewBox="0 0 24 24" fill="currentColor" width="11" height="11"><polygon points="5 3 19 12 5 21 5 3"/></svg>
                    {$t('common.play')}
                  </button>
                </div>
              </div>
            {/each}
          </div>
        {/if}

      <!-- Screenshots tab -->
      {:else if activeTab === 'screenshots'}
        {#if details.screenshots.length === 0}
          <div class="empty-tab">
            <div class="empty-icon-circle">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="28" height="28"><rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/></svg>
            </div>
            <p class="text-muted">{$t('instanceDetail.noScreenshots')}</p>
            <span class="text-muted" style="font-size:11px">{$t('instanceDetail.folder', { path: `${details.game_dir}/screenshots/` })}</span>
          </div>
        {:else}
          <div class="screenshots-header">
            <span class="text-muted" style="font-size:12px">{details.screenshots.length} Screenshot{details.screenshots.length !== 1 ? 's' : ''}</span>
            <button class="btn btn-ghost btn-sm" on:click={openFolder}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="12" height="12"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
              {$t('common.open')}
            </button>
          </div>
          <div class="screenshot-grid">
            {#each details.screenshots as ss, i}
              <!-- svelte-ignore a11y-click-events-have-key-events -->
              <!-- svelte-ignore a11y-no-static-element-interactions -->
              <div class="screenshot-card card card-hover" on:click={() => openGallery(i)}>
                <div class="screenshot-img-wrap">
                  {#if screenshotUrls[ss.path]}
                    <img src={screenshotUrls[ss.path]} alt={ss.filename} class="screenshot-img" />
                  {:else if ss.path in screenshotUrls}
                    <div class="screenshot-placeholder">
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="28" height="28" style="color:var(--border)"><rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/></svg>
                    </div>
                  {:else}
                    <div class="screenshot-placeholder">
                      <div class="dl-spinner" style="width:16px;height:16px;border-width:2px"></div>
                    </div>
                  {/if}
                  <div class="screenshot-overlay">
                    <svg viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" width="18" height="18"><path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7"/></svg>
                  </div>
                </div>
                <div class="screenshot-info">
                  <span class="screenshot-name">{parseScreenshotDate(ss.filename)}</span>
                  <span class="text-muted" style="font-size:10px">{(ss.size_bytes / 1024).toFixed(0)} KB</span>
                </div>
              </div>
            {/each}
          </div>
        {/if}

      <!-- Logs tab -->
      {:else if activeTab === 'logs'}
        {#if liveLogs.length === 0 && !details?.log_tail}
          <div class="empty-tab text-muted">
            {#if isRunning}
              <div class="dl-spinner" style="width:20px;height:20px;border-width:2px"></div>
              {$t('instanceDetail.waitingForLogs')}
            {:else}
              {$t('instanceDetail.noLogs')}
            {/if}
          </div>
        {:else}
          {@const logLines = isRunning ? liveLogs : (details?.log_tail?.split('\n') ?? [])}
          <div class="log-viewer" bind:this={logViewerEl} use:autoScroll>
            {#each logLines as line}
              {@const level = parseLogLine(line)}
              <div class="log-line log-{level}">{line}</div>
            {/each}
          </div>
        {/if}
      {/if}
    {/if}
  </div>
</div>

<!-- ══ Update Progress Popup ══ -->
{#if updatingAll || updatingItem}
  <div class="upd-popup">
    <div class="upd-popup-spinner"></div>
    <div class="upd-popup-text">
      {#if updatingAll}
        <span class="upd-popup-title">Mods werden aktualisiert…</span>
        <span class="upd-popup-sub">{updatingItem ? updatingItem.split(':')[1] : ''}</span>
      {:else}
        <span class="upd-popup-title">Wird aktualisiert…</span>
        <span class="upd-popup-sub">{updatingItem?.split(':')[1] ?? ''}</span>
      {/if}
    </div>
  </div>
{/if}

<!-- ══ Discover Modal (add content) ══ -->
{#if showDiscoverModal && instance}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="discover-modal-overlay" on:click|self={closeDiscoverModal}>
    <div class="discover-modal">
      <Discover
        mode="modal"
        targetInstanceId={instance.id}
        targetVersion={instance.version || null}
        targetLoader={instance.loader !== 'vanilla' ? instance.loader : null}
        defaultCategory={discoverCategory}
        onClose={closeDiscoverModal}
      />
    </div>
  </div>
{/if}

<!-- ══ Screenshot Gallery ══ -->
{#if galleryIdx !== null && details?.screenshots?.length > 0}
  {@const ss = details.screenshots[galleryIdx]}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="gallery-overlay" on:click={closeGallery}>
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div class="gallery-window" on:click|stopPropagation={() => {}}>

      <!-- Top bar -->
      <div class="gallery-topbar">
        <span class="gallery-date">{parseScreenshotDate(ss.filename)}</span>
        <div class="gallery-actions">
          <button class="gallery-btn" on:click={copyGalleryItem} disabled={ssCopying || galleryLoading} title="In Zwischenablage kopieren">
            {#if ssCopying}
              <div class="dl-spinner" style="width:13px;height:13px;border-width:2px"></div>
            {:else}
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
            {/if}
          </button>
          <button class="gallery-btn" on:click={compressGalleryItem} disabled={ssCompressing || galleryLoading} title="Als JPEG komprimieren">
            {#if ssCompressing}
              <div class="dl-spinner" style="width:13px;height:13px;border-width:2px"></div>
            {:else}
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
            {/if}
          </button>
          <button class="gallery-btn gallery-btn-danger" on:click={deleteGalleryItem} title="Löschen">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6m4-6v6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/></svg>
          </button>
          <div class="gallery-divider"></div>
          <button class="gallery-btn" on:click={closeGallery} title="Schließen">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
      </div>

      <!-- Image -->
      <div class="gallery-img-wrap">
        <img
          src={galleryFullSrc || screenshotUrls[ss.path] || ''}
          alt={ss.filename}
          class="gallery-img"
          class:gallery-img-loading={galleryLoading && !screenshotUrls[ss.path]}
        />
        {#if galleryLoading}
          <div class="gallery-loading-badge">
            <div class="dl-spinner" style="width:14px;height:14px;border-width:2px"></div>
          </div>
        {/if}
      </div>

      <!-- Side arrows -->
      {#if details.screenshots.length > 1}
        <button class="gallery-arrow gallery-prev" on:click={prevGallery}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="18" height="18"><polyline points="15 18 9 12 15 6"/></svg>
        </button>
        <button class="gallery-arrow gallery-next" on:click={nextGallery}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="18" height="18"><polyline points="9 18 15 12 9 6"/></svg>
        </button>
      {/if}

      <!-- Footer -->
      <div class="gallery-footer">
        <span class="gallery-filename">{ss.filename}</span>
        <span class="gallery-meta">{(ss.size_bytes / 1024).toFixed(0)} KB &nbsp;·&nbsp; {galleryIdx + 1} / {details.screenshots.length}</span>
      </div>

    </div>
  </div>
{/if}

<!-- ══ Edit Modal ══ -->
{#if editMode}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="modal-overlay" on:click|self={() => (editMode = false)}>
    <div class="edit-modal card">
      <!-- Sidebar -->
      <nav class="edit-nav">
        <div class="edit-nav-title">{$t('instanceDetail.editTitle')}</div>
        {#each [
          { id: 'general',      labelKey: 'instanceDetail.sectionGeneral',      icon: 'M12 2a5 5 0 1 1 0 10A5 5 0 0 1 12 2zm0 12c-5.33 0-8 2.67-8 4v2h16v-2c0-1.33-2.67-4-8-4z' },
          { id: 'installation', labelKey: 'instanceDetail.sectionInstallation', icon: 'M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5' },
          { id: 'window',       labelKey: 'instanceDetail.sectionWindow',       icon: 'M3 3h18v18H3z M3 9h18' },
          { id: 'java',         labelKey: 'instanceDetail.sectionJava',         icon: 'M13 2L3 14h9l-1 8 10-12h-9z' },
          { id: 'hooks',        labelKey: 'instanceDetail.sectionHooks',        icon: 'M5 3l14 9-14 9V3z' },
        ] as sec}
          <button
            class="edit-nav-item"
            class:active={editSection === sec.id}
            on:click={() => editSection = sec.id}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14">
              <path d={sec.icon}/>
            </svg>
            {$t(sec.labelKey)}
          </button>
        {/each}
      </nav>

      <!-- Content -->
      <div class="edit-content">

        <!-- ─ General ─ -->
        {#if editSection === 'general'}
          <div class="edit-section-title">{$t('instanceDetail.sectionGeneral')}</div>

          <div class="form-group">
            <!-- svelte-ignore a11y-label-has-associated-control -->
            <label class="form-label">{$t('instanceDetail.sectionIcon')}</label>
            <div class="icon-upload-row">
              <div class="icon-preview">
                {#if editIcon && editIcon !== 'remove'}
                  <img src={editIcon} alt="" class="icon-preview-img" />
                {:else if iconSrc && editIcon !== 'remove'}
                  <img src={iconSrc} alt="" class="icon-preview-img" />
                {:else}
                  <div class="icon-preview-placeholder" style="color: {getLoaderColor(instance?.loader)}">
                    {instance?.name?.[0]?.toUpperCase() || '?'}
                  </div>
                {/if}
              </div>
              <div class="icon-upload-actions">
                <input type="file" id="icon-file-input" accept="image/*" style="display:none" on:change={handleIconFile} />
                <label for="icon-file-input" class="btn btn-ghost btn-sm" style="cursor:pointer">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="12" height="12"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
                  {$t('instanceDetail.uploadIcon')}
                </label>
                {#if (editIcon && editIcon !== 'remove') || (iconSrc && editIcon !== 'remove')}
                  <button class="btn btn-ghost btn-sm" style="color:var(--text-muted);font-size:11px" on:click={() => { editIcon = 'remove'; editIconSourceUrl = null }}>{$t('common.remove')}</button>
                {/if}
              </div>
            </div>
          </div>

          <div class="form-group">
            <label class="form-label" for="edit-name">{$t('instanceDetail.name')}</label>
            <input id="edit-name" class="input" bind:value={editName} placeholder={$t('instanceDetail.name')} />
          </div>

          <div class="form-group">
            <label class="form-label" for="edit-group">{$t('instanceDetail.group')} <span class="form-label-hint">({$t('common.optional')})</span></label>
            <input id="edit-group" class="input" bind:value={editGroup} placeholder="e.g. Modpacks, Survival…" />
          </div>

          <hr class="divider" />

          <div class="form-group">
            <!-- svelte-ignore a11y-label-has-associated-control -->
            <label class="form-label">{$t('instanceDetail.sectionActions')}</label>
            <div class="action-row">
              <button class="btn btn-ghost btn-sm" on:click={duplicateInstance}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
                {$t('instanceDetail.duplicate')}
              </button>
              <button class="btn btn-sm" style="background:rgba(239,68,68,.1);color:#ef4444;border:1px solid rgba(239,68,68,.3)" on:click={() => { editMode = false; promptDelete() }}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14H6L5 6"/></svg>
                {$t('common.delete')}
              </button>
            </div>
          </div>

          <div class="edit-section-actions">
            <button class="btn btn-ghost" on:click={() => (editMode = false)}>{$t('common.cancel')}</button>
            <button class="btn btn-primary" on:click={saveInstance} disabled={saving}>{saving ? $t('common.saving') : $t('common.save')}</button>
          </div>

        <!-- ─ Installation ─ -->
        {:else if editSection === 'installation'}
          <div class="edit-section-title">{$t('instanceDetail.sectionInstallation')}</div>

          <!-- Current install card -->
          <div class="install-card">
            <div class="install-card-icon" style="color:{getLoaderColor(instance?.loader)}">
              {@html loaderIcon(instance?.loader) || ''}
            </div>
            <div class="install-card-info">
              <div class="install-card-name">{getLoaderName(instance?.loader)}</div>
              <div class="install-card-versions">
                {#if instance?.loader_version}<span>{instance.loader_version}</span><span class="install-card-sep">·</span>{/if}
                <span>MC {instance?.version || '–'}</span>
              </div>
            </div>
            <button class="btn btn-ghost btn-sm install-repair-btn" on:click={repairInstance} disabled={editInstalling}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/></svg>
              {$t('instanceDetail.repair')}
            </button>
          </div>

          <hr class="divider" />
          <div class="form-section-label">{$t('instanceDetail.changeVersion')}</div>

          <div class="form-group">
            <label class="form-label" for="edit-game-version">{$t('instanceDetail.mcVersion')}</label>
            <input id="edit-game-version" class="input" bind:value={editGameVersion} placeholder={instance?.version || '1.21.1'} />
          </div>

          <div class="form-group">
            <!-- svelte-ignore a11y-label-has-associated-control -->
            <label class="form-label">{$t('instanceDetail.modLoader')}</label>
            <Select
              bind:value={editLoader}
              options={['vanilla','fabric','forge','neoforge','quilt'].map(l => ({ value: l, label: getLoaderName(l) }))}
              on:change={() => { editLoaderVersion = ''; fetchEditLoaderVersions() }}
            />
          </div>

          {#if editLoader !== 'vanilla'}
            <div class="form-group">
              <!-- svelte-ignore a11y-label-has-associated-control -->
              <label class="form-label">{$t('instanceDetail.loaderVersion')}</label>
              {#if editLoadingLoaderVersions}
                <div class="text-muted" style="font-size:12px">{$t('instanceDetail.loaderVersionsLoading')}</div>
              {:else}
                <Select
                  bind:value={editLoaderVersion}
                  options={[{ value: '', label: $t('instanceDetail.loaderVersionLatest') }, ...editLoaderVersions.map(v => ({ value: v, label: v }))]}
                />
              {/if}
            </div>
          {/if}

          {#if editInstallError}
            <p class="text-warning" style="font-size:12px">{editInstallError}</p>
          {/if}

          <div class="edit-section-actions">
            <button class="btn btn-ghost" on:click={() => (editMode = false)}>{$t('common.cancel')}</button>
            <button class="btn btn-primary" on:click={applyInstallChanges} disabled={editInstalling}>
              {editInstalling ? $t('common.saving') : $t('instanceDetail.saveChanges')}
            </button>
          </div>

        <!-- ─ Window ─ -->
        {:else if editSection === 'window'}
          <div class="edit-section-title">{$t('instanceDetail.sectionWindow')}</div>

          <div class="form-group">
            <label class="toggle-label">
              <input type="checkbox" class="toggle-check" bind:checked={editFullscreen} />
              <span class="toggle-track"><span class="toggle-thumb"></span></span>
              <span>{$t('instanceDetail.fullscreen')}</span>
            </label>
          </div>

          {#if !editFullscreen}
            <div class="form-row" style="margin-top:8px">
              <div class="form-group" style="flex:1">
                <label class="form-label" for="edit-width">{$t('instanceDetail.width')}</label>
                <input id="edit-width" class="input" type="number" min="320" max="7680" bind:value={editWidth} />
              </div>
              <div class="form-group" style="flex:1">
                <label class="form-label" for="edit-height">{$t('instanceDetail.height')}</label>
                <input id="edit-height" class="input" type="number" min="240" max="4320" bind:value={editHeight} />
              </div>
            </div>
            <div class="preset-row">
              {#each [[854,480,'854×480'],[1280,720,'1280×720'],[1920,1080,'1920×1080']] as [w,h,lbl]}
                <button class="preset-btn" class:active={editWidth===w && editHeight===h} on:click={() => { editWidth=w; editHeight=h }}>
                  {lbl}
                </button>
              {/each}
            </div>
          {/if}

          <div class="edit-section-actions">
            <button class="btn btn-ghost" on:click={() => (editMode = false)}>{$t('common.cancel')}</button>
            <button class="btn btn-primary" on:click={saveInstance} disabled={saving}>{saving ? $t('common.saving') : $t('common.save')}</button>
          </div>

        <!-- ─ Java & Memory ─ -->
        {:else if editSection === 'java'}
          <div class="edit-section-title">{$t('instanceDetail.sectionJava')}</div>

          <div class="form-group">
            <div class="java-auto-info">
              <span class="java-auto-label">{$t('instanceDetail.javaVersion')}</span>
              <span class="java-auto-value text-accent">{autoJavaInfo.label}</span>
            </div>

            <label class="checkbox-row" style="margin-top:10px">
              <input type="checkbox" bind:checked={editCustomJava} />
              <span class="checkbox-label">{$t('instanceDetail.javaCustom')}</span>
            </label>

            {#if editCustomJava}
              <div class="input-with-btn" style="margin-top:8px">
                <input id="edit-java-path" class="input" bind:value={editJavaPath} placeholder="/usr/lib/jvm/java-21/bin/java" style="flex:1;font-family:monospace;font-size:12px" />
                <button class="btn btn-ghost btn-sm" on:click={() => { editJavaPath = '' }}>{$t('instanceDetail.javaAuto')}</button>
              </div>
            {/if}
          </div>

          <hr class="divider" />
          <div class="form-section-label">{$t('instanceDetail.sectionRam')}</div>

          <div class="ram-slider-block">
            <div class="ram-slider-header">
              <span class="form-label">{$t('instanceDetail.ramAlloc')}</span>
              <span class="ram-value text-accent">{formatRam(editRamMax)}</span>
            </div>
            <div class="ram-slider-wrap">
              <input
                type="range"
                class="range-input"
                min="0"
                max={ramSnaps.length - 1}
                value={ramMaxIdx}
                on:input={onRamMaxSlider}
              />
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

          <hr class="divider" />
          <div class="form-section-label">{$t('instanceDetail.sectionJvmArgs')}</div>
          <div class="form-group">
            <textarea
              class="input textarea-mono"
              rows="3"
              placeholder="-XX:+UseG1GC -XX:MaxGCPauseMillis=50"
              bind:value={editCustomJvmArgs}
            ></textarea>
          </div>

          <hr class="divider" />
          <div class="form-section-label" style="margin-bottom:8px">{$t('instanceDetail.sectionEnvVars')}</div>
          {#each editEnvVars as ev, idx}
            <div class="env-row">
              <input class="input env-input" placeholder="KEY" bind:value={ev.key} />
              <span class="env-eq">=</span>
              <input class="input env-input" placeholder="value" bind:value={ev.value} />
              <button class="icon-btn" on:click={() => editEnvVars = editEnvVars.filter((_, i) => i !== idx)}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
              </button>
            </div>
          {/each}
          <button class="btn btn-ghost btn-sm" style="width:fit-content;margin-top:4px" on:click={() => editEnvVars = [...editEnvVars, { key: '', value: '' }]}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="12" height="12"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
            {$t('instanceDetail.addEnvVar')}
          </button>

          <div class="edit-section-actions">
            <button class="btn btn-ghost" on:click={() => (editMode = false)}>{$t('common.cancel')}</button>
            <button class="btn btn-primary" on:click={saveInstance} disabled={saving}>{saving ? $t('common.saving') : $t('common.save')}</button>
          </div>

        <!-- ─ Launch Hooks ─ -->
        {:else if editSection === 'hooks'}
          <div class="edit-section-title">{$t('instanceDetail.sectionHooks')}</div>
          <p class="hook-hint">{$t('instanceDetail.hooksDesc', { placeholder: '%INSTANCE_DIR%' })}</p>

          <div class="form-group">
            <label class="form-label" for="edit-pre-launch">{$t('instanceDetail.preLaunch')} <span class="form-label-hint">({$t('instanceDetail.preLaunchHint')})</span></label>
            <input id="edit-pre-launch" class="input input-mono" bind:value={editPreLaunch} placeholder="e.g. notify-send 'Minecraft starting'" />
          </div>

          <div class="form-group">
            <label class="form-label" for="edit-wrapper">{$t('instanceDetail.wrapper')} <span class="form-label-hint">({$t('instanceDetail.wrapperHint')})</span></label>
            <input id="edit-wrapper" class="input input-mono" bind:value={editWrapper} placeholder="e.g. gamescope -W 1920 -H 1080" />
          </div>

          <div class="form-group">
            <label class="form-label" for="edit-post-exit">{$t('instanceDetail.postExit')} <span class="form-label-hint">({$t('instanceDetail.postExitHint')})</span></label>
            <input id="edit-post-exit" class="input input-mono" bind:value={editPostExit} placeholder="e.g. notify-send 'Minecraft closed'" />
          </div>

          <div class="edit-section-actions">
            <button class="btn btn-ghost" on:click={() => (editMode = false)}>{$t('common.cancel')}</button>
            <button class="btn btn-primary" on:click={saveInstance} disabled={saving}>{saving ? $t('common.saving') : $t('common.save')}</button>
          </div>
        {/if}

      </div>
    </div>
  </div>
{/if}

{#if launchingId}
  <LaunchOverlay instanceName={instance?.name ?? ''} />
{/if}

<!-- ══ Destructive Delete Modal ══ -->
{#if showDeleteModal}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="modal-overlay destructive-overlay" on:click|self={() => (showDeleteModal = false)}>
    <div class="modal card destructive-modal">
      <div class="destructive-icon">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="32" height="32">
          <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
          <line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
        </svg>
      </div>
      <h2 class="destructive-title">{$t('instanceDetail.deleteConfirm')}</h2>
      <p class="destructive-body">
        {$t('instanceDetail.deleteBody', { name: instance?.name ?? '' })}
      </p>
      <div class="destructive-actions">
        <button class="btn btn-ghost" on:click={() => (showDeleteModal = false)}>{$t('common.cancel')}</button>
        <button class="btn btn-delete" on:click={confirmDelete}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14H6L5 6"/></svg>
          {$t('instanceDetail.deleteFinal')}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  @font-face {
    font-family: 'Minecraft';
    src: url('https://cdn.jsdelivr.net/gh/South-Paw/typeface-minecraft@master/files/minecraft.woff2') format('woff2'),
         url('https://cdn.jsdelivr.net/gh/South-Paw/typeface-minecraft@master/files/minecraft.woff') format('woff');
    font-weight: normal;
    font-style: normal;
  }

  .header-left { display: flex; align-items: center; gap: 14px; }
  .back-btn { padding: 6px; border-radius: var(--radius-sm); color: var(--text-muted); display: flex; align-items: center; }
  .back-btn:hover { background: var(--surface2); color: var(--text); }
  .instance-title-block { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .instance-meta { display: flex; align-items: center; gap: 5px; flex-wrap: wrap; }
  .meta-badge { display: inline-flex; align-items: center; gap: 4px; padding: 3px 9px; border-radius: 100px; font-size: 11px; font-weight: 500; background: var(--surface3); border: 1px solid var(--border); color: var(--text-dim); flex-shrink: 0; }
  .loader-icon { display: flex; align-items: center; width: 12px; height: 12px; }
  .loader-icon :global(svg) { width: 12px; height: 12px; }
  .header-actions { display: flex; align-items: center; gap: 8px; }

  /* Instance icon in header */
  .instance-icon-wrap { width: 52px; height: 52px; flex-shrink: 0; border-radius: 10px; overflow: hidden; border: 1.5px solid var(--border); box-shadow: 0 2px 8px rgba(0,0,0,0.25); }
  .instance-icon-img { width: 100%; height: 100%; object-fit: cover; display: block; image-rendering: pixelated; }
  .instance-icon-placeholder { width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; font-size: 18px; font-weight: 700; }

  /* Running/downloading states */
  .running-pill { display: inline-flex; align-items: center; gap: 6px; padding: 4px 10px; border-radius: 100px; background: rgba(52,211,153,0.1); border: 1px solid rgba(52,211,153,0.25); font-size: 12px; color: var(--success); }
  .running-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--success); }

  .status-pill { display: inline-flex; align-items: center; gap: 6px; padding: 4px 10px; border-radius: 100px; font-size: 12px; max-width: 220px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .status-pill.downloading { background: rgba(var(--accent-rgb),0.1); border: 1px solid rgba(var(--accent-rgb),0.25); color: var(--accent); }
  .dl-spinner { width: 10px; height: 10px; border: 1.5px solid rgba(var(--accent-rgb),0.3); border-top-color: var(--accent); border-radius: 50%; animation: spin 0.7s linear infinite; flex-shrink: 0; }
  @keyframes spin { to { transform: rotate(360deg) } }

  .btn-stop { background: rgba(248,113,113,0.12); border: 1px solid rgba(248,113,113,0.3); color: var(--error); display: inline-flex; align-items: center; gap: 6px; padding: 5px 12px; border-radius: var(--radius-sm); font-size: 13px; font-weight: 500; cursor: pointer; transition: all var(--transition); }
  .btn-stop:hover { background: rgba(248,113,113,0.2); border-color: var(--error); }

  .tabs { display: flex; gap: 2px; padding: 0 24px; border-bottom: 1px solid var(--border); flex-shrink: 0; overflow: visible; }
  .tab { padding: 11px 16px; font-size: 13px; font-weight: 500; color: var(--text-muted); border-bottom: 2px solid transparent; margin-bottom: -1px; transition: all var(--transition); white-space: nowrap; }
  .tab:hover { color: var(--text); }
  .tab.active { color: var(--accent); border-bottom-color: var(--accent); font-weight: 600; }

  .loading-state, .error-state { padding: 48px; text-align: center; font-size: 13px; }
  .empty-tab { padding: 48px 20px; text-align: center; font-size: 13px; display: flex; flex-direction: column; align-items: center; gap: 14px; }
  .empty-icon-circle { width: 64px; height: 64px; border-radius: 50%; background: rgba(var(--accent-rgb),0.07); border: 1px solid rgba(var(--accent-rgb),0.15); display: flex; align-items: center; justify-content: center; color: var(--accent); }

  /* Content tab */
  .content-tab { display: flex; flex-direction: column; gap: 0; height: 100%; }

  .content-section-bar {
    display: flex; align-items: center;
    padding: 12px 0 14px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .content-chip-group { display: flex; gap: 5px; flex-wrap: wrap; }
  .content-chip {
    display: inline-flex; align-items: center; gap: 5px;
    padding: 5px 12px;
    border-radius: 100px;
    border: 1px solid var(--border);
    background: var(--surface2);
    font-size: 12px; color: var(--text-dim);
    cursor: pointer; transition: all 0.12s ease; white-space: nowrap;
  }
  .content-chip:hover { border-color: var(--text-muted); color: var(--text); }
  .content-chip.active {
    border-color: var(--accent); color: var(--accent);
    background: color-mix(in srgb, var(--accent) 8%, var(--surface2));
  }
  .tab-update-badge {
    display: inline-flex; align-items: center; justify-content: center;
    min-width: 16px; height: 15px; padding: 0 4px;
    background: rgba(251,191,36,0.15); border: 1px solid rgba(251,191,36,0.4);
    color: var(--warning, #f59e0b); border-radius: 8px; font-size: 10px; font-weight: 700;
    margin-left: 4px;
  }
  .chip-upd-cnt {
    display: inline-flex; align-items: center; justify-content: center;
    min-width: 14px; height: 14px; padding: 0 3px;
    background: rgba(251,191,36,0.15); border: 1px solid rgba(251,191,36,0.4);
    color: var(--warning, #f59e0b); border-radius: 8px; font-size: 9px; font-weight: 700;
  }
  .chip-cnt {
    display: inline-flex; align-items: center; justify-content: center;
    min-width: 16px; height: 15px; padding: 0 4px;
    background: var(--surface3); border-radius: 8px;
    font-size: 10px; font-weight: 600; color: var(--text-muted);
  }
  .content-chip.active .chip-cnt { background: rgba(var(--accent-rgb),0.15); color: var(--accent); }

  .section-content { flex: 1; }
  .section-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px; padding-top: 16px; }
  .section-title-block { display: flex; align-items: baseline; gap: 7px; }
  .section-subtitle { font-size: 16px; font-weight: 700; color: var(--text); }
  .section-subtitle-count {
    font-size: 11px; font-weight: 600; color: var(--text-muted);
    background: var(--surface2); border: 1px solid var(--border);
    border-radius: 100px; padding: 1px 7px;
  }

  /* Mods */
  .mod-list { display: flex; flex-direction: column; gap: 6px; }
  .mod-row { display: flex; align-items: center; gap: 14px; padding: 12px 14px; box-shadow: 0 1px 3px rgba(0,0,0,0.1); }
  .mod-icon-wrap { width: 48px; height: 48px; flex-shrink: 0; border-radius: 10px; overflow: hidden; background: var(--surface2); display: flex; align-items: center; justify-content: center; border: 1px solid var(--border); box-shadow: 0 1px 3px rgba(0,0,0,0.15); }
  .mod-icon-img { width: 48px; height: 48px; object-fit: contain; display: block; image-rendering: pixelated; }
  .mod-icon-placeholder { width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; color: var(--text-muted); }
  .mod-info { flex: 1; display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .mod-name { font-size: 13px; font-weight: 500; color: var(--text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .mod-filename { font-size: 10px; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font-family: monospace; }
  .mod-size { font-size: 11px; color: var(--text-muted); }
  .icon-btn { padding: 6px; border-radius: var(--radius-sm); display: flex; align-items: center; }
  .icon-btn:hover { background: var(--surface2); color: var(--error); }

  /* Update hint (subtle inline row) */
  .update-hint { display: flex; align-items: center; gap: 7px; margin-bottom: 8px; font-size: 11.5px; color: rgba(251,191,36,0.6); }
  .update-hint span { flex: 1; }

  /* Update chip in header */
  .upd-chip { color: rgba(251,191,36,0.75) !important; background: rgba(251,191,36,0.08) !important; border-color: rgba(251,191,36,0.2) !important; }

  /* Per-mod update button */
  .mod-row.has-update { border-color: rgba(251,191,36,0.25); }
  .update-btn { background: rgba(251,191,36,0.1); border: 1px solid rgba(251,191,36,0.3); color: var(--warning, #f59e0b); display: inline-flex; align-items: center; gap: 5px; padding: 4px 9px; border-radius: var(--radius-sm); font-size: 11px; font-weight: 500; cursor: pointer; transition: all var(--transition); white-space: nowrap; }
  .update-btn:hover:not(:disabled) { background: rgba(251,191,36,0.2); border-color: rgba(251,191,36,0.5); }
  .update-btn:disabled { opacity: 0.6; cursor: not-allowed; }
  .upd-spinner { width: 10px; height: 10px; border: 1.5px solid rgba(251,191,36,0.3); border-top-color: var(--warning, #f59e0b); border-radius: 50%; animation: spin 0.7s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg) } }

  /* Worlds */
  .worlds-list { display: flex; flex-direction: column; gap: 8px; }
  .world-row { display: flex; align-items: center; gap: 12px; padding: 10px 12px; }
  .world-icon { color: var(--accent); flex-shrink: 0; width: 32px; height: 32px; display: flex; align-items: center; justify-content: center; }
  .world-icon-img { width: 32px; height: 32px; object-fit: cover; border-radius: 4px; image-rendering: pixelated; }
  .world-info { flex: 1; display: flex; flex-direction: column; gap: 2px; }
  .world-name { font-size: 13px; color: var(--text); }
  .world-date { font-size: 11px; }

  /* Servers */
  .server-row { display: grid; grid-template-columns: 32px 160px 1fr auto; align-items: center; gap: 12px; padding: 10px 12px; }
  .server-name-col { display: flex; flex-direction: column; gap: 2px; overflow: hidden; }
  .server-name-col .world-name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .server-name-col .world-date { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .server-motd-col { display: flex; align-items: center; justify-content: center; }
  .server-motd { font-family: 'Minecraft', monospace; font-size: 11px; text-align: center; white-space: pre-wrap; line-height: 1.6; word-break: break-word; }
  .server-right { display: flex; align-items: center; gap: 8px; justify-content: flex-end; }
  .server-players { display: flex; align-items: center; gap: 4px; font-size: 12px; color: var(--text); white-space: nowrap; }
  .server-offline { font-size: 11px; color: #f87171; }

  /* Screenshots */
  .screenshots-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px; }
  .screenshot-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 12px; }
  .screenshot-card { overflow: hidden; cursor: pointer; }
  .screenshot-img-wrap { position: relative; overflow: hidden; aspect-ratio: 16/9; background: var(--surface2); }
  .screenshot-img { width: 100%; height: 100%; object-fit: cover; display: block; transition: transform 0.2s ease; }
  .screenshot-card:hover .screenshot-img { transform: scale(1.04); }
  .screenshot-placeholder { width: 100%; height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 6px; }
  .screenshot-overlay { position: absolute; inset: 0; background: rgba(0,0,0,0); display: flex; align-items: center; justify-content: center; opacity: 0; transition: all 0.2s ease; }
  .screenshot-card:hover .screenshot-overlay { background: rgba(0,0,0,0.4); opacity: 1; }
  .screenshot-info { padding: 8px 10px; display: flex; align-items: center; justify-content: space-between; }
  .screenshot-name { font-size: 11px; color: var(--text-dim); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 120px; }

  /* Logs */
  .log-viewer { background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 10px; max-height: calc(100vh - 220px); overflow-y: auto; font-family: monospace; font-size: 11px; line-height: 1.6; }
  .log-line { padding: 0 2px; white-space: pre-wrap; word-break: break-all; }
  .log-default { color: var(--text-muted); }
  .log-info    { color: var(--text-dim); }
  .log-debug   { color: var(--text-muted); opacity: 0.6; }
  .log-warn    { color: var(--warning, #f59e0b); }
  .log-error   { color: var(--error); font-weight: 500; }

  /* Discover modal overlay */
  .discover-modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.75); z-index: 100; display: flex; align-items: center; justify-content: center; padding: 12px; }
  .discover-modal { width: 100%; max-width: 1100px; height: 90vh; background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); overflow: hidden; display: flex; flex-direction: column; }

  /* Gallery */
  .gallery-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.93); z-index: 150; display: flex; align-items: center; justify-content: center; padding: 20px; }
  .gallery-window { position: relative; display: flex; flex-direction: column; align-items: center; gap: 10px; max-width: min(90vw, 1400px); width: 100%; }
  .gallery-topbar { width: 100%; display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .gallery-date { font-size: 13px; font-weight: 600; color: rgba(255,255,255,0.65); }
  .gallery-actions { display: flex; align-items: center; gap: 3px; }
  .gallery-btn { width: 30px; height: 30px; border-radius: var(--radius-sm); border: none; background: rgba(255,255,255,0.07); color: rgba(255,255,255,0.6); cursor: pointer; display: flex; align-items: center; justify-content: center; transition: background 0.12s, color 0.12s; }
  .gallery-btn:hover:not(:disabled) { background: rgba(255,255,255,0.15); color: white; }
  .gallery-btn:disabled { opacity: 0.35; cursor: not-allowed; }
  .gallery-btn-danger:hover:not(:disabled) { background: rgba(248,113,113,0.18); color: #f87171; }
  .gallery-divider { width: 1px; height: 18px; background: rgba(255,255,255,0.12); margin: 0 3px; }
  .gallery-img-wrap { position: relative; display: flex; align-items: center; justify-content: center; max-height: calc(90vh - 110px); width: 100%; }
  .gallery-img { max-width: 100%; max-height: calc(90vh - 110px); object-fit: contain; display: block; border-radius: var(--radius); }
  .gallery-img-loading { opacity: 0.4; }
  .gallery-loading-badge { position: absolute; bottom: 10px; right: 10px; background: rgba(0,0,0,0.55); border-radius: var(--radius-sm); padding: 5px 7px; display: flex; align-items: center; }
  .gallery-arrow { position: fixed; top: 50%; transform: translateY(-50%); background: rgba(255,255,255,0.09); border: none; color: rgba(255,255,255,0.7); cursor: pointer; width: 44px; height: 44px; border-radius: 50%; display: flex; align-items: center; justify-content: center; transition: background 0.15s, color 0.15s; z-index: 10; }
  .gallery-arrow:hover { background: rgba(255,255,255,0.18); color: white; }
  .gallery-prev { left: 16px; }
  .gallery-next { right: 16px; }
  .gallery-footer { width: 100%; display: flex; align-items: center; justify-content: space-between; font-size: 11px; color: rgba(255,255,255,0.3); }
  .gallery-filename { font-family: monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 60%; }
  .gallery-meta { flex-shrink: 0; }

  /* Edit modal */
  .modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; z-index: 100; }
  .edit-modal { display: flex; flex-direction: row; width: 760px; max-width: 95vw; height: 520px; max-height: 90vh; overflow: hidden; padding: 0; gap: 0; }

  /* Sidebar nav */
  .edit-nav { width: 178px; flex-shrink: 0; padding: 20px 12px; display: flex; flex-direction: column; gap: 2px; border-right: 1px solid var(--border); background: var(--surface2); height: 100%; box-sizing: border-box; }
  .edit-nav-title { font-size: 11px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.08em; color: var(--text-muted); padding: 0 6px 10px; }
  .edit-nav-item { display: flex; align-items: center; gap: 8px; padding: 7px 10px; border-radius: var(--radius-sm); font-size: 12.5px; color: var(--text-dim); text-align: left; cursor: pointer; transition: background 0.12s, color 0.12s; background: none; border: none; width: 100%; }
  .edit-nav-item:hover { background: var(--surface3); color: var(--text); }
  .edit-nav-item.active { background: rgba(var(--accent-rgb),0.12); color: var(--accent); font-weight: 600; }

  /* Main content */
  .edit-content { flex: 1; overflow-y: auto; padding: 24px; display: flex; flex-direction: column; gap: 14px; min-width: 0; height: 100%; box-sizing: border-box; }
  .edit-section-title { font-size: 15px; font-weight: 700; margin-bottom: 4px; }
  .edit-section-actions { display: flex; justify-content: flex-end; gap: 8px; padding-top: 8px; margin-top: auto; }

  .form-group { display: flex; flex-direction: column; gap: 6px; }
  .form-row { display: flex; gap: 12px; }
  .form-label { font-size: 12px; color: var(--text-dim); }
  .form-label-hint { color: var(--text-muted); font-weight: 400; }
  .form-section-label { font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.1em; color: var(--text-muted); padding-bottom: 2px; border-bottom: 1px solid var(--border); }
  .range-input { accent-color: var(--accent); cursor: pointer; width: 100%; }
  .divider { border: none; border-top: 1px solid var(--border); margin: 0; }
  .btn-sm { padding: 5px 10px; font-size: 12px; }

  /* Icon upload */
  .icon-upload-row { display: flex; align-items: center; gap: 12px; }
  .icon-preview { width: 56px; height: 56px; border-radius: 10px; overflow: hidden; border: 1px solid var(--border); background: var(--surface2); flex-shrink: 0; }
  .icon-preview-img { width: 100%; height: 100%; object-fit: cover; display: block; image-rendering: pixelated; }
  .icon-preview-placeholder { width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; font-size: 22px; font-weight: 700; }
  .icon-upload-actions { display: flex; flex-direction: column; gap: 6px; }

  /* Actions row in General */
  .action-row { display: flex; gap: 8px; flex-wrap: wrap; }

  /* Installation card */
  .install-card { display: flex; align-items: center; gap: 14px; padding: 12px 14px; background: var(--surface2); border: 1px solid var(--border); border-radius: var(--radius); }
  .install-card-icon { width: 38px; height: 38px; flex-shrink: 0; display: flex; align-items: center; justify-content: center; }
  .install-card-icon :global(svg) { width: 38px; height: 38px; }
  .install-card-info { flex: 1; display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .install-card-name { font-size: 14px; font-weight: 600; color: var(--text); }
  .install-card-versions { display: flex; align-items: center; gap: 5px; font-size: 12px; color: var(--text-muted); }
  .install-card-sep { opacity: 0.5; }
  .install-repair-btn { flex-shrink: 0; }

  /* RAM slider */
  .ram-slider-block { display: flex; flex-direction: column; gap: 6px; }
  .ram-slider-header { display: flex; align-items: center; justify-content: space-between; }
  .ram-value { font-size: 13px; font-weight: 600; }
  .ram-slider-wrap { position: relative; }
  .ram-ticks { position: relative; height: 5px; margin-top: 1px; }
  .ram-tick { position: absolute; top: 0; width: 1.5px; height: 5px; background: var(--border); transform: translateX(-50%); border-radius: 1px; }
  .ram-marks { position: relative; height: 16px; margin-top: 2px; }
  .ram-mark { position: absolute; transform: translateX(-50%); font-size: 10px; color: var(--text-muted); top: 0; white-space: nowrap; }
  .ram-mark--first { transform: translateX(0); }
  .ram-mark--last  { transform: translateX(-100%); }

  /* Toggle */
  .toggle-label { display: flex; align-items: center; gap: 10px; cursor: pointer; font-size: 13px; color: var(--text-dim); user-select: none; }
  .toggle-check { display: none; }
  .toggle-track { width: 36px; height: 20px; background: var(--surface3); border-radius: 10px; position: relative; flex-shrink: 0; border: 1px solid var(--border); transition: background 0.2s; }
  .toggle-check:checked + .toggle-track { background: var(--accent); border-color: var(--accent); }
  .toggle-thumb { position: absolute; top: 2px; left: 2px; width: 14px; height: 14px; background: #fff; border-radius: 50%; transition: left 0.2s; }
  .toggle-check:checked + .toggle-track .toggle-thumb { left: 18px; }

  /* Resolution presets */
  .preset-row { display: flex; gap: 6px; flex-wrap: wrap; margin-top: 4px; }
  .preset-btn { padding: 4px 10px; border-radius: var(--radius-sm); font-size: 11px; border: 1px solid var(--border); background: var(--surface2); color: var(--text-dim); cursor: pointer; transition: all 0.12s; }
  .preset-btn:hover { background: var(--surface3); color: var(--text); }
  .preset-btn.active { background: rgba(var(--accent-rgb),0.12); color: var(--accent); border-color: rgba(var(--accent-rgb),0.35); }

  /* Input with button */
  .input-with-btn { display: flex; gap: 6px; }

  /* Env vars */
  .env-row { display: flex; align-items: center; gap: 6px; margin-bottom: 4px; }
  .env-input { flex: 1; min-width: 0; }
  .env-eq { color: var(--text-muted); font-family: monospace; font-weight: 600; flex-shrink: 0; }

  /* Monospace textarea/input */
  .textarea-mono { font-family: monospace; font-size: 12px; resize: vertical; }
  .input-mono { font-family: monospace; font-size: 12px; }

  /* Hook hint */
  .hook-hint { font-size: 12px; color: var(--text-muted); margin: 0; line-height: 1.5; }

  /* ── Three-dot context menu ── */
  .ctx-wrap { position: relative; }

  .ctx-trigger { padding: 7px 9px; border: none; background: transparent; }
  .ctx-trigger:hover { color: var(--text); }

  .ctx-backdrop {
    position: fixed;
    inset: 0;
    z-index: 190;
  }

  .ctx-menu {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 200;
    background: var(--surface2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 4px;
    min-width: 160px;
    box-shadow: 0 8px 24px rgba(0,0,0,0.35);
    animation: ctxIn 0.12s ease;
  }

  @keyframes ctxIn {
    from { opacity: 0; transform: translateY(-4px) scale(0.97) }
    to   { opacity: 1; transform: translateY(0)   scale(1)    }
  }

  .ctx-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 10px;
    border-radius: var(--radius-sm);
    font-size: 13px;
    color: var(--text-dim);
    text-align: left;
    transition: background 0.1s, color 0.1s;
    cursor: pointer;
  }

  .ctx-item:hover {
    background: var(--surface3);
    color: var(--text);
  }

  .ctx-item-danger { color: #ef4444; }
  .ctx-item-danger:hover { background: rgba(239,68,68,0.1); color: #ef4444; }

  .ctx-divider {
    height: 1px;
    background: var(--border);
    margin: 4px 0;
  }

  /* ── Destructive modal ── */
  .destructive-overlay {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .destructive-modal {
    max-width: 380px;
    width: calc(100% - 32px);
    padding: 28px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    text-align: center;
    border-color: rgba(239, 68, 68, 0.35);
    box-shadow: 0 0 0 1px rgba(239, 68, 68, 0.2), 0 0 40px rgba(239, 68, 68, 0.1), 0 8px 32px rgba(0,0,0,0.4);
    animation: destructiveIn 0.2s cubic-bezier(0.34, 1.4, 0.64, 1);
  }

  @keyframes destructiveIn {
    from { transform: scale(0.92); opacity: 0 }
    to   { transform: scale(1);    opacity: 1 }
  }

  .destructive-icon {
    width: 56px;
    height: 56px;
    border-radius: 50%;
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.25);
    display: flex;
    align-items: center;
    justify-content: center;
    color: #ef4444;
  }

  .destructive-title {
    font-size: 16px;
    font-weight: 700;
    color: var(--text);
    margin: 0;
  }

  .destructive-body {
    font-size: 13px;
    color: var(--text-dim);
    margin: 0;
    line-height: 1.5;
  }

  .destructive-actions {
    display: flex;
    gap: 10px;
    margin-top: 4px;
    width: 100%;
    justify-content: center;
  }

  .btn-delete {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    border-radius: var(--radius-sm);
    font-size: 13px;
    font-weight: 600;
    background: rgba(239, 68, 68, 0.15);
    color: #ef4444;
    border: 1px solid rgba(239, 68, 68, 0.35);
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-delete:hover {
    background: rgba(239, 68, 68, 0.25);
    border-color: rgba(239, 68, 68, 0.6);
    box-shadow: 0 0 12px rgba(239, 68, 68, 0.2);
  }

  /* ── Mod toggle switch ── */
  .mod-disabled { opacity: 0.45; }
  .mod-toggle-btn {
    flex-shrink: 0;
    width: 34px; height: 19px;
    border-radius: 100px;
    background: var(--surface3);
    border: 1.5px solid var(--border);
    position: relative;
    cursor: pointer;
    transition: background 0.18s, border-color 0.18s;
    padding: 0;
  }
  .mod-toggle-btn.mod-toggle-on {
    background: rgba(var(--accent-rgb), 0.85);
    border-color: var(--accent);
  }
  .mod-toggle-btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .mod-toggle-thumb {
    position: absolute;
    top: 2px; left: 2px;
    width: 13px; height: 13px;
    border-radius: 50%;
    background: #fff;
    transition: left 0.18s;
    pointer-events: none;
  }
  .mod-toggle-on .mod-toggle-thumb { left: 17px; }

  /* ── Crash analysis panel ── */
  .crash-panel {
    background: rgba(239,68,68,0.07);
    border: 1px solid rgba(239,68,68,0.3);
    border-radius: var(--radius);
    padding: 14px 16px;
    margin-bottom: 12px;
    display: flex; flex-direction: column; gap: 10px;
  }
  .crash-panel-header {
    display: flex; align-items: center; gap: 8px;
    font-size: 13px; font-weight: 700;
    color: var(--error, #ef4444);
  }
  .crash-issue {
    background: rgba(0,0,0,0.15);
    border-radius: calc(var(--radius) - 2px);
    padding: 10px 12px;
    display: flex; flex-direction: column; gap: 4px;
    border-left: 3px solid var(--error, #ef4444);
  }
  .crash-issue-warn { border-left-color: var(--warning, #f59e0b); }
  .crash-issue-title { font-size: 12px; font-weight: 700; color: var(--text); }
  .crash-issue-desc  { font-size: 11.5px; color: var(--text-dim); line-height: 1.45; }
  .crash-issue-fix   {
    font-size: 11px; color: var(--accent);
    display: flex; align-items: flex-start; gap: 5px;
    line-height: 1.45; padding-top: 2px;
  }


  /* Update progress popup */
  .upd-popup {
    position: absolute;
    bottom: 28px;
    right: 28px;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 20px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: 0 8px 32px rgba(0,0,0,0.4);
    z-index: 200;
    min-width: 240px;
  }
  .upd-popup-spinner {
    width: 22px; height: 22px; flex-shrink: 0;
    border: 2.5px solid rgba(var(--accent-rgb),0.15);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg) } }
  .upd-popup-text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .upd-popup-title { font-size: 13px; font-weight: 600; color: var(--text); }
  .upd-popup-sub { font-size: 11px; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  /* Java version selector in instance edit */
  .java-auto-info { display: flex; flex-direction: column; gap: 3px; padding: 10px 12px; background: var(--surface2); border-radius: var(--radius); border: 1px solid var(--border); }
  .java-auto-label { font-size: 11px; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.05em; }
  .java-auto-value { font-size: 13px; font-weight: 500; font-family: 'JetBrains Mono', monospace; word-break: break-all; }
  .checkbox-row { display: flex; align-items: center; gap: 8px; cursor: pointer; user-select: none; }
  .checkbox-label { font-size: 13px; color: var(--text); }
</style>
