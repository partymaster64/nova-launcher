<script>
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { get } from 'svelte/store'
  import { config, manifest, currentPage, isOnline } from '../../store.js'
  import { loaderIcon, loaderColor } from '../loaderIcons.js'
  import Select from '../components/Select.svelte'
  import Checkbox from '../components/Checkbox.svelte'
  import { t } from '../i18n.js'

  // Props — when used as modal from InstanceDetail
  export let mode = 'page'           // 'page' | 'modal'
  export let targetInstanceId = null
  export let targetVersion = null
  export let targetLoader = null
  export let defaultCategory = 'mod'
  export let onClose = null

  let cfg = null
  let mfst = null
  config.subscribe(v => (cfg = v))
  manifest.subscribe(v => (mfst = v))

  // Categories
  const CATEGORIES = [
    { id: 'modpack',     label: 'Modpacks',       icon: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><rect x="1" y="3" width="15" height="13"/><polygon points="16 8 20 8 23 11 23 16 16 16 16 8"/><circle cx="5.5" cy="18.5" r="2.5"/><circle cx="18.5" cy="18.5" r="2.5"/></svg>`, canInstall: true },
    { id: 'mod',         label: 'Mods',            icon: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/></svg>`, canInstall: true },
    { id: 'resourcepack',label: 'Resource Packs',  icon: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 9h6v6H9z"/></svg>`, canInstall: true },
    { id: 'datapack',    label: 'Data Packs',      icon: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>`, canInstall: true },
    { id: 'shader',      label: 'Shader Packs',    icon: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>`, canInstall: true },
  ]

  let category = defaultCategory
  let query = ''
  let gameVersion = ''
  let hits = []
  let totalHits = 0
  let offset = 0
  let loading = false
  let searchTimeout = null
  let viewModes = { modpack: 'card', mod: 'list', resourcepack: 'card', datapack: 'list', shader: 'card' }
  $: viewMode = viewModes[category] ?? 'card'
  function setViewMode(mode) { viewModes = { ...viewModes, [category]: mode } }

  // Project detail
  let detailProject = null
  let detailHit = null
  let detailVersions = []    // ALL versions, unfiltered
  let loadingDetail = false
  let installingVersion = null
  let installToast = null
  let detailTab = 'overview'  // 'overview' | 'versions' | 'gallery'

  // Modpack install state: { [versionId]: 'installing' | 'done' | 'error' }
  let modpackInstallState = {}
  // Wizard modal state for modpack install
  let modWizardOpen  = false
  let modWizardStep  = ''
  let modWizardPct   = 0
  let modWizardDone  = false
  let modWizardError = null

  // Compatibility filter — on by default in modal mode when an instance is active
  let filterCompatible = true

  // Instance picker dialog
  let showInstancePicker = false
  let pickerPendingHit = null     // hit to quick-install after picking instance
  let pickerPendingVersion = null // specific version to install after picking instance
  let pickerSelectedId = targetInstanceId

  onMount(() => {
    search()
  })

  $: releaseVersions = mfst?.versions?.filter(v => v.type === 'release').slice(0, 30) || []

  // In modal mode targetInstanceId is fixed; in page mode it comes from picker
  $: effectiveInstanceId = targetInstanceId || pickerSelectedId || null
  $: effectiveInstance = cfg?.instances?.find(i => i.id === effectiveInstanceId) || null
  $: effectiveVersion = targetVersion || effectiveInstance?.version || gameVersion || null
  $: effectiveLoader = targetLoader || (effectiveInstance?.loader && effectiveInstance.loader !== 'vanilla' ? effectiveInstance.loader : null) || null

  // For vanilla instances (no loader), mods and shaders can't be installed
  $: instanceIsVanilla = effectiveInstance != null && (!effectiveInstance.loader || effectiveInstance.loader === 'vanilla')
  $: visibleCategories = instanceIsVanilla
    ? CATEGORIES.filter(c => c.id !== 'mod' && c.id !== 'shader')
    : CATEGORIES
  // Auto-redirect to resourcepack if current category becomes unavailable
  $: if (instanceIsVanilla && (category === 'mod' || category === 'shader')) category = 'resourcepack'
  $: currentCategory = visibleCategories.find(c => c.id === category) || visibleCategories[0]

  $: galleryImages = [
    ...(detailProject?.gallery?.filter(g => g.url) || []),
    ...(detailHit?.featured_gallery ? [{ url: detailHit.featured_gallery, title: '' }] : []),
  ]
  $: if (detailProject) { galleryIndex = 0; detailTab = 'overview' }

  // Versions shown in the list — filter by compat when toggle is on
  $: displayedVersions = (filterCompatible && effectiveInstance)
    ? detailVersions.filter(v => checkCompat(v, effectiveInstance).ok)
    : detailVersions

  // Reset filterCompatible to true whenever a new detail is opened
  $: if (detailHit) { filterCompatible = true }

  let galleryIndex = 0
  function prevGallery() { if (galleryIndex > 0) galleryIndex-- }
  function nextGallery() { if (galleryIndex < galleryImages.length - 1) galleryIndex++ }

  // ── Markdown renderer ──────────────────────────────────────────────────────
  function renderMarkdown(text) {
    if (!text) return ''

    // Protect fenced code blocks before any processing
    const codeBlocks = []
    text = text.replace(/```[\w]*\r?\n?([\s\S]*?)```/g, (_, code) => {
      const idx = codeBlocks.length
      const escaped = code.trim().replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
      codeBlocks.push(`<pre class="md-code-block"><code>${escaped}</code></pre>`)
      return `\x00CODE${idx}\x00`
    })

    const lines = text.split('\n')
    const result = []
    let inList = false
    let listType = 'ul'
    let inParagraph = false

    function closeList() {
      if (inList) { result.push(listType === 'ol' ? '</ol>' : '</ul>'); inList = false }
    }
    function closeParagraph() {
      if (inParagraph) { result.push('</p>'); inParagraph = false }
    }

    for (let i = 0; i < lines.length; i++) {
      const raw = lines[i]
      const trimmed = raw.trim()

      // Restore protected code block
      if (/^\x00CODE\d+\x00$/.test(trimmed)) {
        closeParagraph(); closeList()
        result.push(codeBlocks[parseInt(trimmed.match(/\d+/)[0])])
        continue
      }

      // Raw HTML block — pass through unchanged (handles <center>, <h1 id=...>, <img>, <sup>, etc.)
      if (/^<\/?[a-zA-Z]/.test(trimmed)) {
        closeParagraph(); closeList()
        result.push(raw)
        continue
      }

      // Headings
      const h1m = trimmed.match(/^# (.+)/)
      if (h1m) { closeParagraph(); closeList(); result.push(`<h2 class="md-h2">${inlineMarkdown(h1m[1])}</h2>`); continue }
      const h2m = trimmed.match(/^## (.+)/)
      if (h2m) { closeParagraph(); closeList(); result.push(`<h3 class="md-h3">${inlineMarkdown(h2m[1])}</h3>`); continue }
      const h3m = trimmed.match(/^#{3,} (.+)/)
      if (h3m) { closeParagraph(); closeList(); result.push(`<h4 class="md-h4">${inlineMarkdown(h3m[1])}</h4>`); continue }

      // Blockquote
      if (/^> /.test(trimmed)) {
        closeParagraph(); closeList()
        result.push(`<blockquote class="md-quote">${inlineMarkdown(trimmed.slice(2))}</blockquote>`)
        continue
      }

      // Horizontal rule
      if (/^---+$/.test(trimmed) || /^\*\*\*+$/.test(trimmed)) {
        closeParagraph(); closeList(); result.push('<hr class="md-hr">'); continue
      }

      // Unordered list
      if (/^[*\-] /.test(trimmed)) {
        closeParagraph()
        if (!inList || listType !== 'ul') { closeList(); result.push('<ul class="md-list">'); inList = true; listType = 'ul' }
        result.push(`<li>${inlineMarkdown(trimmed.slice(2))}</li>`)
        continue
      }

      // Ordered list
      if (/^\d+\. /.test(trimmed)) {
        closeParagraph()
        if (!inList || listType !== 'ol') { closeList(); result.push('<ol class="md-list">'); inList = true; listType = 'ol' }
        result.push(`<li>${inlineMarkdown(trimmed.replace(/^\d+\.\s/, ''))}</li>`)
        continue
      }

      // Empty line
      if (trimmed === '') { closeList(); closeParagraph(); continue }

      // Regular paragraph text
      closeList()
      if (!inParagraph) { result.push('<p class="md-p">'); inParagraph = true }
      result.push(inlineMarkdown(trimmed))
    }

    closeList()
    closeParagraph()
    return result.join('\n')
  }

  function escHtml(s) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  }

  function inlineMarkdown(text) {
    // Escape HTML entities in plain text portions, but handle images/links first
    // Process images before escaping so their URLs survive
    const parts = []
    let rest = text

    // Split out markdown images ![alt](url) and links [text](url) to protect URLs from escaping
    rest = rest.replace(/!\[([^\]]*)\]\(([^)]+)\)/g, (_, alt, url) => {
      const idx = parts.length
      const isImage = /\.(png|jpe?g|gif|svg|webp|avif)(\?|$)/i.test(url)
        || /^https?:\/\/(cdn\.|img\.|raw\.|shields\.io|badge\.fury|badgen\.net)/i.test(url)
      if (isImage) {
        parts.push(`<img class="md-img" src="${url}" alt="${escHtml(alt)}" loading="lazy" />`)
      } else {
        const safeUrl = url.replace(/'/g, '%27')
        const label = alt || url
        parts.push(`<a class="md-badge" href="${url}" onclick="event.preventDefault();window.__novaOpenUrl&&window.__novaOpenUrl('${safeUrl}')">${escHtml(label)}</a>`)
      }
      return `\x01IMG${idx}\x01`
    })
    rest = rest.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_, label, url) => {
      const idx = parts.length
      const safeUrl = url.replace(/'/g, '%27')
      parts.push(`<a class="md-link" href="${url}" onclick="event.preventDefault();window.__novaOpenUrl&&window.__novaOpenUrl('${safeUrl}')">${escHtml(label)}</a>`)
      return `\x01LINK${idx}\x01`
    })

    // Now escape remaining HTML
    rest = escHtml(rest)

    // Apply inline formatting
    rest = rest
      .replace(/\*\*\*(.+?)\*\*\*/g, '<strong><em>$1</em></strong>')
      .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
      .replace(/__(.+?)__/g, '<strong>$1</strong>')
      .replace(/\*(.+?)\*/g, '<em>$1</em>')
      .replace(/_(.+?)_/g, '<em>$1</em>')
      .replace(/`([^`]+)`/g, '<code class="md-code">$1</code>')
      .replace(/~~(.+?)~~/g, '<del>$1</del>')

    // Restore protected parts — LINK first so nested ![img](url) inside [text](url) also resolves
    rest = rest.replace(/\x01LINK(\d+)\x01/g, (_, i) => parts[parseInt(i)])
    rest = rest.replace(/\x01IMG(\d+)\x01/g, (_, i) => parts[parseInt(i)])
    return rest
  }

  // Expose open_url for inline links
  if (typeof window !== 'undefined') {
    window.__novaOpenUrl = (url) => invoke('open_url', { url })
  }

  // ── Compatibility check ────────────────────────────────────────────────────
  function checkCompat(ver, instance) {
    if (!instance) return { ok: true, gameVersionOk: true, loaderOk: true }
    const gv = instance.version
    const loader = instance.loader && instance.loader !== 'vanilla' ? instance.loader.toLowerCase() : null

    const gameVersionOk = !gv || ver.game_versions?.some(v => v === gv)
    const loaderOk = !loader || ver.loaders?.some(l => l.toLowerCase() === loader)
    return { ok: gameVersionOk && loaderOk, gameVersionOk, loaderOk }
  }

  // ── Search ─────────────────────────────────────────────────────────────────
  function onQueryChange() {
    clearTimeout(searchTimeout)
    searchTimeout = setTimeout(() => { offset = 0; search() }, 400)
  }

  async function search() {
    loading = true
    hits = []
    try {
      const result = await invoke('search_modrinth_projects', {
        query,
        projectType: category,
        gameVersion: (gameVersion || effectiveVersion) || null,
        loader: (category === 'mod' ? effectiveLoader : null) || null,
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

  function changeCategory(id) {
    category = id
    offset = 0
    hits = []
    search()
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

  // ── Detail view ────────────────────────────────────────────────────────────
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
    detailProject = null
    detailHit = null
    detailVersions = []
    galleryIndex = 0
  }

  // ── Install logic ──────────────────────────────────────────────────────────
  async function installVersion(ver) {
    // Check compat warning
    const compat = checkCompat(ver, effectiveInstance)
    if (!compat.ok) {
      const tFn = get(t)
      const details =
        (!compat.gameVersionOk ? tFn('discover.incompatibleDetails_gameVersion', { version: effectiveInstance.version }) + '\n' : '') +
        (!compat.loaderOk ? tFn('discover.incompatibleDetails_loader', { loader: effectiveInstance.loader }) + '\n' : '')
      const proceed = confirm(tFn('discover.incompatibleWarning', { details }))
      if (!proceed) return
    }

    if (!effectiveInstanceId) {
      pickerPendingVersion = ver
      showInstancePicker = true
      return
    }
    await doInstallVersion(ver, effectiveInstanceId)
  }

  async function doInstallVersion(ver, instanceId) {
    installingVersion = ver.id
    try {
      await invoke('install_content', {
        projectId: detailHit.project_id,
        versionId: ver.id,
        instanceId,
        contentType: category,
        title: detailHit.title,
        iconUrl: detailHit.icon_url || null,
      })
      showInstallToast(get(t)('discover.installed', { title: detailHit.title }))
    } catch (e) {
      showInstallToast(get(t)('discover.installError', { error: String(e) }), 'error')
    } finally {
      installingVersion = null
    }
  }

  async function installLatest(hit) {
    if (!effectiveInstanceId && mode === 'page') {
      pickerPendingHit = hit
      showInstancePicker = true
      return
    }
    await doInstallLatest(hit, effectiveInstanceId)
  }

  async function doInstallLatest(hit, instanceId) {
    installingVersion = hit.project_id
    try {
      const inst = cfg?.instances?.find(i => i.id === instanceId)
      const gv = targetVersion || inst?.version || null
      const loader = targetLoader || (inst?.loader && inst.loader !== 'vanilla' ? inst.loader : null) || null

      // Try compatible version first
      let versions = await invoke('get_modrinth_versions', {
        projectId: hit.project_id,
        gameVersion: gv,
        loader: category === 'mod' ? loader : null,
      })

      if (!versions?.length && inst) {
        // No compatible version — try unfiltered and warn
        const all = await invoke('get_modrinth_versions', {
          projectId: hit.project_id,
          gameVersion: null,
          loader: null,
        })
        if (!all?.length) {
          showInstallToast(get(t)('discover.noVersionAvailable'), 'error')
          return
        }
        const tFn = get(t)
        const lines = []
        if (gv) lines.push(tFn('discover.incompatibleDetails_gameVersion', { version: gv }))
        if (loader && category === 'mod') lines.push(tFn('discover.incompatibleDetails_loader', { loader }))
        const proceed = confirm(tFn('discover.noCompatibleWarning', { details: lines.join('\n') }))
        if (!proceed) return
        versions = all
      } else if (!versions?.length) {
        showInstallToast(get(t)('discover.noCompatibleVersion'), 'error')
        return
      }

      await invoke('install_content', {
        projectId: hit.project_id,
        versionId: versions[0].id,
        instanceId,
        contentType: category,
        title: hit.title,
        iconUrl: hit.icon_url || null,
      })
      showInstallToast(get(t)('discover.installed', { title: hit.title }))
    } catch (e) {
      showInstallToast(get(t)('discover.installError', { error: String(e) }), 'error')
    } finally {
      installingVersion = null
    }
  }

  // ── Instance picker ────────────────────────────────────────────────────────
  function openPicker(hit) {
    pickerPendingHit = hit
    pickerPendingVersion = null
    pickerSelectedId = null
    showInstancePicker = true
  }

  async function confirmPicker() {
    if (!pickerSelectedId) return
    showInstancePicker = false
    if (pickerPendingVersion) {
      await doInstallVersion(pickerPendingVersion, pickerSelectedId)
    } else if (pickerPendingHit) {
      await doInstallLatest(pickerPendingHit, pickerSelectedId)
    }
    pickerPendingHit = null
    pickerPendingVersion = null
  }

  function closePicker() {
    showInstancePicker = false
    pickerPendingHit = null
    pickerPendingVersion = null
  }

  // ── Modpack Install ─────────────────────────────────────────────────────────
  function pollModpackProgress(instanceId, onStep) {
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

  async function installModpack(ver) {
    if (modpackInstallState[ver.id]) return
    modpackInstallState[ver.id] = 'installing'
    modpackInstallState = modpackInstallState

    modWizardOpen  = true
    modWizardDone  = false
    modWizardError = null
    modWizardStep  = 'Erstelle Instanz…'
    modWizardPct   = 0

    try {
      const instanceId = await invoke('install_modpack', {
        versionId: ver.id,
        name: detailHit?.title || null,
        iconUrl: detailHit?.icon_url || null,
      })

      invoke('get_config').then(cfg => config.set(cfg))
      await pollModpackProgress(instanceId, (step, pct) => {
        modWizardStep = step
        modWizardPct  = pct
      })

      modWizardStep = 'Minecraft wird installiert…'
      modWizardPct  = 0
      await invoke('prepare_instance', { instanceId })
      await pollModpackProgress(instanceId, (step, pct) => {
        modWizardStep = step
        modWizardPct  = pct
      })

      const finalCfg = await invoke('get_config')
      config.set(finalCfg)

      modpackInstallState[ver.id] = 'done'
      modpackInstallState = modpackInstallState
      modWizardDone = true

      setTimeout(() => {
        modWizardOpen = false
        currentPage.set('instances')
      }, 1800)
    } catch (e) {
      modpackInstallState[ver.id] = 'error'
      modpackInstallState = modpackInstallState
      modWizardError = String(e)
    }
  }

  // ── Helpers ────────────────────────────────────────────────────────────────
  function showInstallToast(msg, type = 'success') {
    installToast = { msg, type }
    setTimeout(() => (installToast = null), 3500)
  }

  function formatNum(n) {
    if (n >= 1000000) return `${(n / 1000000).toFixed(1)}M`
    if (n >= 1000) return `${(n / 1000).toFixed(0)}K`
    return String(n)
  }

  function formatDate(iso) {
    try { return new Date(iso).toLocaleDateString('de-DE') } catch { return iso }
  }

  function versionTypeBadgeColor(t) {
    return t === 'release' ? 'var(--success)' : t === 'beta' ? 'var(--warning, #f59e0b)' : 'var(--text-muted)'
  }

  // Banner layout: modpack, resourcepack, shader get card+banner; mod, datapack get compact list
  $: useBannerLayout = viewMode === 'card'
</script>

<div class="discover-wrap" class:is-modal={mode === 'modal'}>
  {#if !$isOnline}
    <div class="offline-overlay">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="40" height="40"><line x1="1" y1="1" x2="23" y2="23"/><path d="M16.72 11.06A10.94 10.94 0 0 1 19 12.55M5 12.55a10.94 10.94 0 0 1 5.17-2.39M10.71 5.05A16 16 0 0 1 22.56 9M1.42 9a15.91 15.91 0 0 1 4.7-2.88M8.53 16.11a6 6 0 0 1 6.95 0M12 20h.01"/></svg>
      <span class="offline-overlay-title">Kein Internet</span>
      <span class="offline-overlay-sub">Entdecken ist offline nicht verfügbar.</span>
    </div>
  {:else}
  <!-- ── Header ── -->
  <div class="discover-header">
    <div class="header-left">
      {#if mode === 'modal'}
        <button class="back-btn" on:click={onClose}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        </button>
      {/if}
      <div>
        <h1 class="page-title">{mode === 'modal' ? $t('discover.addTitle') : $t('discover.title')}</h1>
        {#if mode === 'modal' && targetInstanceId}
          <span class="text-muted" style="font-size:12px">{cfg?.instances?.find(i => i.id === targetInstanceId)?.name || ''}</span>
        {/if}
      </div>
    </div>
    <div class="header-right">
      <span class="text-muted" style="font-size:12px">{$t('mods.results', { count: totalHits.toLocaleString() })}</span>
    </div>
  </div>

  <!-- ── Category Tabs ── -->
  <div class="category-tabs">
    {#each visibleCategories as cat}
      <button
        class="cat-tab"
        class:active={category === cat.id}
        on:click={() => changeCategory(cat.id)}
      >
        <span class="cat-icon">{@html cat.icon}</span>
        {cat.label}
      </button>
    {/each}
  </div>

  <!-- ── Search Bar ── -->
  <div class="search-bar">
    <div class="search-input-wrap">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="15" height="15" class="search-icon"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
      <input
        class="search-input"
        type="text"
        placeholder={$t('discover.searchPlaceholder')}
        bind:value={query}
        on:input={onQueryChange}
      />
    </div>
    {#if mode === 'page'}
      <div class="filter-select">
        <Select
          bind:value={gameVersion}
          options={[{ value: '', label: $t('discover.allVersions') }, ...releaseVersions.map(v => ({ value: v.id, label: v.id }))]}
          on:change={() => { offset = 0; search() }}
        />
      </div>
    {/if}
    <div class="view-toggle">
      <button class="view-btn" class:active={viewMode === 'card'} on:click={() => setViewMode('card')} title="Kachelansicht">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="15" height="15"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/></svg>
      </button>
      <button class="view-btn" class:active={viewMode === 'list'} on:click={() => setViewMode('list')} title="Listenansicht">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="15" height="15"><line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="12" x2="21" y2="12"/><line x1="3" y1="18" x2="21" y2="18"/></svg>
      </button>
    </div>
  </div>

  <!-- ── Content Grid ── -->
  <div class="discover-body">
    {#if loading}
      {#if useBannerLayout}
        <div class="results-grid">
          {#each Array(12) as _}
            <div class="result-card card skeleton"></div>
          {/each}
        </div>
      {:else}
        <div class="results-list">
          {#each Array(10) as _}
            <div class="compact-row-skeleton card skeleton"></div>
          {/each}
        </div>
      {/if}
    {:else if hits.length === 0}
      <div class="empty-state text-muted">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="56" height="56"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
        <p>{$t('discover.noResults')}</p>
      </div>
    {:else}
      {#if useBannerLayout}
        <div class="results-grid">
          {#each hits as hit}
            <!-- svelte-ignore a11y-click-events-have-key-events -->
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <div class="result-card card card-hover" on:click={() => openDetail(hit)}>
              <div class="card-banner">
                {#if hit.featured_gallery}
                  <img src={hit.featured_gallery} alt="" class="card-banner-img" />
                {:else}
                  <div class="card-banner-fallback" style="background: {hit.color ? `#${Math.abs(hit.color).toString(16).padStart(6,'0')}18` : 'var(--surface2)'}">
                    {#if hit.icon_url}
                      <img src={hit.icon_url} alt="" class="banner-fallback-icon" />
                    {:else}
                      <span class="banner-letter">{hit.title[0]}</span>
                    {/if}
                  </div>
                {/if}
              </div>
              {#if hit.icon_url && hit.featured_gallery}
                <img src={hit.icon_url} alt="" class="card-icon-overlay" />
              {/if}

              <div class="card-body" class:has-icon={!!(hit.icon_url && hit.featured_gallery)}>
                <div class="card-title-wrap">
                  <span class="card-title">{hit.title}</span>
                  <span class="card-author text-muted">{hit.author || ''}</span>
                </div>

                <p class="card-desc text-muted">{hit.description}</p>

                <div class="card-tags">
                  {#each hit.categories.slice(0, 3) as cat}
                    <span class="badge badge-muted">{cat}</span>
                  {/each}
                  {#if hit.versions?.length > 0}
                    <span class="badge badge-info">{hit.versions[hit.versions.length - 1]}</span>
                  {/if}
                </div>

                <div class="card-footer">
                  <span class="dl-count text-muted">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                    {formatNum(hit.downloads)}
                  </span>
                  {#if currentCategory.canInstall && category !== 'modpack'}
                    <!-- svelte-ignore a11y-click-events-have-key-events -->
                    <button
                      class="btn btn-primary btn-sm install-btn"
                      disabled={!!installingVersion}
                      on:click|stopPropagation={() => installLatest(hit)}
                    >
                      {#if installingVersion === hit.project_id}
                        <div class="btn-spinner"></div>
                      {:else}
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                      {/if}
                      {$t('discover.addTitle')}
                    </button>
                  {/if}
                </div>
              </div>
            </div>
          {/each}
        </div>
      {:else}
        <!-- Compact list (mod / datapack) -->
        <div class="results-list">
          {#each hits as hit}
            <!-- svelte-ignore a11y-click-events-have-key-events -->
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <div class="compact-row card card-hover" on:click={() => openDetail(hit)}>
              <div class="compact-icon">
                {#if hit.icon_url}
                  <img src={hit.icon_url} alt="" class="compact-icon-img" />
                {:else}
                  <div class="compact-icon-placeholder">{hit.title[0]}</div>
                {/if}
              </div>
              <div class="compact-info">
                <div class="compact-title-row">
                  <span class="card-title">{hit.title}</span>
                  <span class="card-author text-muted">&nbsp;·&nbsp;{hit.author || ''}</span>
                </div>
                <p class="compact-desc text-muted">{hit.description}</p>
                <div class="compact-tags">
                  {#each hit.categories.slice(0, 3) as cat}
                    <span class="badge badge-muted">{cat}</span>
                  {/each}
                  {#if hit.versions?.length > 0}
                    <span class="badge badge-info">{hit.versions[hit.versions.length - 1]}</span>
                  {/if}
                </div>
              </div>
              <div class="compact-actions">
                <span class="dl-count text-muted">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                  {formatNum(hit.downloads)}
                </span>
                {#if currentCategory.canInstall && category !== 'modpack'}
                  <!-- svelte-ignore a11y-click-events-have-key-events -->
                  <button
                    class="btn btn-primary btn-sm install-btn"
                    disabled={!!installingVersion}
                    on:click|stopPropagation={() => installLatest(hit)}
                  >
                    {#if installingVersion === hit.project_id}
                      <div class="btn-spinner"></div>
                    {:else}
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                    {/if}
                    {$t('discover.addBtn')}
                  </button>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Pagination -->
      <div class="pagination">
        <button class="btn btn-ghost btn-sm" on:click={prevPage} disabled={offset === 0}>{$t('common.back')}</button>
        <span class="page-info text-muted">{$t('common.pagination', { start: offset + 1, end: Math.min(offset + 20, totalHits), total: totalHits.toLocaleString() })}</span>
        <button class="btn btn-ghost btn-sm" on:click={nextPage} disabled={offset + 20 >= totalHits}>{$t('common.next')}</button>
      </div>
    {/if}
  </div>
  {/if}
</div>

<!-- ══════════════════════════════════════════════════════
     Project Detail Overlay
═══════════════════════════════════════════════════════ -->
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
        {#if currentCategory.canInstall}
          {#if category === 'modpack'}
            <button
              class="btn btn-primary btn-sm"
              on:click={() => detailTab = 'versions'}
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="12" height="12"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
              Installieren
            </button>
          {:else}
            <button
              class="btn btn-primary btn-sm"
              disabled={!!installingVersion}
              on:click={() => installLatest(detailHit)}
            >
              {#if installingVersion === detailHit.project_id}
                <div class="btn-spinner"></div>{$t('mods.installing')}
              {:else}
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="12" height="12"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                {$t('discover.addTitle')}
              {/if}
            </button>
          {/if}
        {/if}
        {#if loadingDetail}<div class="detail-spinner"></div>{/if}
      </div>
    </div>

    <!-- Tab bar -->
    <div class="detail-tabs">
      <button class="detail-tab" class:active={detailTab === 'overview'} on:click={() => (detailTab = 'overview')}>{$t('discover.tabOverview')}</button>
      {#if currentCategory.canInstall}
        <button class="detail-tab" class:active={detailTab === 'versions'} on:click={() => (detailTab = 'versions')}>
          {$t('versions.title')}
          {#if detailVersions.length > 0}<span class="tab-badge">{detailVersions.length}</span>{/if}
        </button>
      {/if}
      {#if galleryImages.length > 0}
        <button class="detail-tab" class:active={detailTab === 'gallery'} on:click={() => (detailTab = 'gallery')}>
          {$t('discover.gallery') || 'Gallery'}
          <span class="tab-badge">{galleryImages.length}</span>
        </button>
      {/if}
    </div>

    <!-- Tab content -->
    <div class="detail-body">

      <!-- ── Übersicht ── -->
      {#if detailTab === 'overview'}
        <div class="tab-pane">
          <!-- Short description -->
          <p class="detail-short-desc">{detailHit.description}</p>

          <!-- Rendered markdown body -->
          {#if detailProject?.body}
            <div class="md-body">
              {@html renderMarkdown(detailProject.body)}
            </div>
          {:else if loadingDetail}
            <div class="text-muted" style="font-size:13px;padding:16px 0">{$t('discover.loadingDesc')}</div>
          {/if}

          <!-- Categories & Links -->
          {#if detailHit.categories?.length > 0}
            <div class="overview-section">
              <span class="section-label">{$t('discover.categories') || 'Kategorien'}</span>
              <div class="overview-tags">
                {#each detailHit.categories as cat}
                  <span class="badge badge-muted">{cat}</span>
                {/each}
              </div>
            </div>
          {/if}

          {#if detailProject}
            <div class="overview-section">
              <span class="section-label">{$t('discover.links') || 'Links'}</span>
              <div class="detail-links">
                <a href={`https://modrinth.com/${detailHit.project_type}/${detailProject.slug}`}
                   on:click|preventDefault={() => invoke('open_url', { url: `https://modrinth.com/${detailHit.project_type}/${detailProject.slug}` })}
                   class="detail-link">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
                  Modrinth
                </a>
                {#if detailProject.source_url}
                  <a href={detailProject.source_url} on:click|preventDefault={() => invoke('open_url', { url: detailProject.source_url })} class="detail-link">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>
                    {$t('discover.sourceCode')}
                  </a>
                {/if}
                {#if detailProject.issues_url}
                  <a href={detailProject.issues_url} on:click|preventDefault={() => invoke('open_url', { url: detailProject.issues_url })} class="detail-link">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
                    Issues
                  </a>
                {/if}
                {#if detailProject.discord_url}
                  <a href={detailProject.discord_url} on:click|preventDefault={() => invoke('open_url', { url: detailProject.discord_url })} class="detail-link">
                    Discord
                  </a>
                {/if}
              </div>
            </div>
          {/if}
        </div>

      <!-- ── Versionen ── -->
      {:else if detailTab === 'versions'}
        <div class="tab-pane">
          {#if currentCategory.canInstall}
            {#if mode === 'modal' && effectiveInstance}
              <div class="compat-context">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13"><rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>
                <span>{effectiveInstance.name}</span>
                <span class="text-muted">·</span>
                <span class="text-muted">{effectiveInstance.version || '?'}</span>
                {#if effectiveLoader}<span class="text-muted">·</span><span class="text-muted">{effectiveLoader}</span>{/if}
              </div>
            {/if}

            {#if effectiveInstance}
              <div class="compat-filter-row">
                <Checkbox bind:checked={filterCompatible}>{$t('discover.compatibleOnly')}</Checkbox>
                {#if filterCompatible && detailVersions.length > 0 && displayedVersions.length === 0}
                  <span class="compat-filter-none text-muted">{$t('discover.noCompatible')}</span>
                {:else if filterCompatible}
                  <span class="text-muted" style="font-size:11px">{$t('discover.versionCount', { showing: displayedVersions.length, total: detailVersions.length })}</span>
                {/if}
              </div>
            {/if}

            {#if loadingDetail}
              <div class="text-muted" style="font-size:13px;padding:16px 0">{$t('discover.loadingVersions')}</div>
            {:else if displayedVersions.length === 0 && detailVersions.length === 0}
              <div class="text-muted" style="font-size:13px;padding:16px 0">{$t('discover.noVersions')}</div>
            {:else if displayedVersions.length === 0}
              <div class="text-muted" style="font-size:13px;padding:16px 0">
                {$t('discover.noCompatible')} —
                <button class="link-btn" on:click={() => filterCompatible = false}>{$t('discover.showAll')}</button>
              </div>
            {:else}
              <div class="versions-list">
                {#each displayedVersions as ver}
                  {@const compat = checkCompat(ver, effectiveInstance)}
                  {@const mpState = modpackInstallState[ver.id]}
                  <div class="version-row" class:compat-ok={category !== 'modpack' && effectiveInstance && compat.ok} class:compat-warn={category !== 'modpack' && effectiveInstance && !compat.ok}>
                    <!-- Left: version info -->
                    <div class="version-left">
                      <div class="version-top-row">
                        <span class="version-number">{ver.version_number}</span>
                        <span class="version-type-badge" style="color:{versionTypeBadgeColor(ver.version_type)};border-color:{versionTypeBadgeColor(ver.version_type)}22;background:{versionTypeBadgeColor(ver.version_type)}11">
                          {ver.version_type}
                        </span>
                        {#if category !== 'modpack' && effectiveInstance}
                          {#if compat.ok}
                            <span class="compat-badge compat-ok-badge">{$t('discover.compatible')}</span>
                          {:else}
                            <span class="compat-badge compat-warn-badge">
                              {!compat.gameVersionOk ? `⚠ MC ${effectiveInstance.version}` : `⚠ ${effectiveLoader}`}
                            </span>
                          {/if}
                        {/if}
                      </div>
                      {#if ver.name && ver.name !== ver.version_number}
                        <span class="version-name text-muted">{ver.name}</span>
                      {/if}
                      <div class="version-tags-row">
                        {#each (ver.game_versions || []).slice(0, 5) as gv}
                          <span class="ver-tag">{gv}</span>
                        {/each}
                        {#if (ver.game_versions || []).length > 5}
                          <span class="ver-tag-more text-muted">+{ver.game_versions.length - 5}</span>
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
                    <!-- Right: install -->
                    <div class="version-right">
                      {#if category === 'modpack'}
                        {#if mpState === 'done'}
                          <span class="compat-badge compat-ok-badge">✓ Installiert</span>
                        {:else}
                          <button
                            class="btn btn-primary btn-sm"
                            disabled={!!mpState}
                            on:click={() => installModpack(ver)}
                          >
                            {#if mpState === 'installing'}
                              <div class="btn-spinner"></div> …
                            {:else}
                              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                              {$t('mods.install')}
                            {/if}
                          </button>
                        {/if}
                      {:else}
                        <button
                          class="btn btn-primary btn-sm"
                          class:btn-warn={effectiveInstance && !compat.ok}
                          disabled={!!installingVersion}
                          on:click={() => installVersion(ver)}
                        >
                          {#if installingVersion === ver.id}
                            <div class="btn-spinner"></div>...
                          {:else}
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                            {$t('mods.install')}
                          {/if}
                        </button>
                      {/if}
                    </div>
                  </div>
                {/each}
              </div>
            {/if}
          {/if}
        </div>

      <!-- ── Galerie ── -->
      {:else if detailTab === 'gallery'}
        <div class="tab-pane gallery-pane">
          {#if galleryImages.length === 0}
            <div class="text-muted" style="padding:32px;text-align:center">{$t('discover.noGallery')}</div>
          {:else}
            <!-- Main large image -->
            <div class="gallery-main">
              <img src={galleryImages[galleryIndex].url} alt={galleryImages[galleryIndex].title || ''} class="gallery-img" />
              {#if galleryImages[galleryIndex].title}
                <div class="gallery-caption">{galleryImages[galleryIndex].title}</div>
              {/if}
              {#if galleryImages.length > 1}
                <button class="gallery-nav gallery-prev" on:click={prevGallery} disabled={galleryIndex === 0}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="16" height="16"><polyline points="15 18 9 12 15 6"/></svg>
                </button>
                <button class="gallery-nav gallery-next" on:click={nextGallery} disabled={galleryIndex >= galleryImages.length - 1}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="16" height="16"><polyline points="9 18 15 12 9 6"/></svg>
                </button>
              {/if}
            </div>
            <!-- Thumbnail strip -->
            {#if galleryImages.length > 1}
              <div class="gallery-grid">
                {#each galleryImages as img, i}
                  <button class="gallery-thumb" class:active={i === galleryIndex} on:click={() => (galleryIndex = i)}>
                    <img src={img.url} alt="" class="gallery-thumb-img" />
                  </button>
                {/each}
              </div>
            {/if}
          {/if}
        </div>
      {/if}

    </div>
  </div>
</div>
{/if}

<!-- ══ Instance Picker Dialog ══ -->
{#if showInstancePicker}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="picker-overlay" on:click|self={closePicker}>
    <div class="picker-dialog card">
      <div class="picker-header">
        <h3 class="picker-title">{$t('discover.addToInstance')}</h3>
        <button class="icon-btn" on:click={closePicker}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="15" height="15"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        </button>
      </div>
      {#if pickerPendingHit || pickerPendingVersion}
        <div class="picker-item-name text-muted">
          {pickerPendingHit?.title || detailHit?.title || ''}
          {pickerPendingVersion ? ` · ${pickerPendingVersion.version_number}` : ''}
        </div>
      {/if}
      <div class="picker-list">
        {#each (cfg?.instances || []) as inst}
          <!-- svelte-ignore a11y-click-events-have-key-events -->
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div
            class="picker-inst"
            class:selected={pickerSelectedId === inst.id}
            on:click={() => pickerSelectedId = inst.id}
          >
            <div class="picker-inst-dot" class:active={pickerSelectedId === inst.id}></div>
            <div class="picker-inst-info">
              <span class="picker-inst-name">{inst.name}</span>
              <span class="picker-inst-meta text-muted">{inst.version || '?'} · {inst.loader || 'vanilla'}</span>
            </div>
          </div>
        {/each}
        {#if !cfg?.instances?.length}
          <p class="text-muted" style="font-size:13px;padding:12px 0">{$t('discover.noInstances')}</p>
        {/if}
      </div>
      <div class="picker-actions">
        <button class="btn btn-ghost" on:click={closePicker}>{$t('common.cancel')}</button>
        <button class="btn btn-primary" disabled={!pickerSelectedId} on:click={confirmPicker}>
          {$t('discover.addTitle')}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Toast -->
{#if installToast}
  <div class="install-toast" class:toast-error={installToast.type === 'error'}>{installToast.msg}</div>
{/if}

<!-- ═══ Modpack Install Wizard Modal ═══ -->
{#if modWizardOpen}
<div class="wiz-overlay">
  <div class="wiz-modal">
    <div class="wiz-header">
      <div class="wiz-title">Modpack wird installiert</div>
    </div>
    <div class="wiz-body">
      {#if modWizardDone}
        <div class="wiz-done">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="44" height="44" style="color:var(--success)"><circle cx="12" cy="12" r="10"/><polyline points="8 12 11 15 16 9"/></svg>
          <p class="wiz-done-text">Instanz bereit!</p>
        </div>
      {:else if modWizardError}
        <div class="wiz-error">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="40" height="40" style="color:var(--error)"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          <p class="wiz-error-text">{modWizardError}</p>
          <button class="btn btn-ghost" on:click={() => modWizardOpen = false}>Schließen</button>
        </div>
      {:else}
        <div class="wiz-progress">
          <div class="wiz-logo-wrap">
            <svg viewBox="0 0 100 100" width="80" height="80" xmlns="http://www.w3.org/2000/svg">
              <defs>
                <linearGradient id="disc-sg" x1="0" y1="0" x2="1" y2="1">
                  <stop offset="0%"   stop-color="#f0abfc"/>
                  <stop offset="50%"  stop-color="#a855f7"/>
                  <stop offset="100%" stop-color="#5b21b6"/>
                </linearGradient>
              </defs>
              <g class="wiz-star">
                <path class="wp wpn" d="M50,50 Q40,40 50,7  Q60,40 50,50Z" fill="url(#disc-sg)"/>
                <path class="wp wpe" d="M50,50 Q60,40 93,50 Q60,60 50,50Z" fill="url(#disc-sg)"/>
                <path class="wp wps" d="M50,50 Q60,60 50,93 Q40,60 50,50Z" fill="url(#disc-sg)"/>
                <path class="wp wpw" d="M50,50 Q40,60 7,50  Q40,40 50,50Z" fill="url(#disc-sg)"/>
                <path class="wiz-inner"
                  d="M50,27 Q55,45 70,50 Q55,55 50,73 Q45,55 30,50 Q45,45 50,27Z"
                  fill="white" opacity="0.9"/>
                <circle class="wiz-dot" cx="50" cy="50" r="4.5" fill="white"/>
              </g>
            </svg>
          </div>
          <div class="wiz-step">{modWizardStep || '…'}</div>
          <div class="wiz-bar-wrap">
            <div class="wiz-bar-fill" style="width:{Math.round(modWizardPct * 100)}%"></div>
          </div>
          <div class="wiz-pct">{Math.round(modWizardPct * 100)}%</div>
        </div>
      {/if}
    </div>
  </div>
</div>
{/if}

<style>
  /* ── Wrapper ── */
  .discover-wrap { display: flex; flex-direction: column; height: 100%; overflow: hidden; }

  .offline-overlay { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; flex: 1; height: 100%; color: #fb923c; }
  .offline-overlay-title { font-size: 15px; font-weight: 700; color: #fb923c; }
  .offline-overlay-sub { font-size: 13px; color: var(--text-muted); }

  /* ── Header ── */
  .discover-header { display: flex; align-items: center; justify-content: space-between; padding: 20px 24px 16px; border-bottom: 1px solid var(--border); flex-shrink: 0; gap: 12px; background: linear-gradient(to bottom, color-mix(in srgb, var(--accent) 5%, var(--surface)), var(--surface)); }
  .header-left { display: flex; align-items: center; gap: 10px; }
  .header-right { display: flex; align-items: center; gap: 10px; }
  .back-btn { padding: 6px; border-radius: var(--radius-sm); color: var(--text-muted); display: flex; align-items: center; gap: 4px; font-size: 13px; }
  .back-btn:hover { background: var(--surface2); color: var(--text); }

  /* ── Category Tabs ── */
  .category-tabs { display: flex; gap: 2px; padding: 0 24px; border-bottom: 1px solid var(--border); flex-shrink: 0; overflow-x: auto; }
  .cat-tab { display: flex; align-items: center; gap: 6px; padding: 11px 14px; font-size: 13px; font-weight: 500; color: var(--text-muted); border-bottom: 2px solid transparent; margin-bottom: -1px; transition: all var(--transition); white-space: nowrap; }
  .cat-tab:hover { color: var(--text); }
  .cat-tab.active { color: var(--accent); border-bottom-color: var(--accent); font-weight: 600; }
  .cat-icon { display: flex; align-items: center; opacity: 0.7; }
  .cat-tab.active .cat-icon { opacity: 1; }

  /* ── Search Bar ── */
  .search-bar { display: flex; gap: 10px; padding: 12px 24px; border-bottom: 1px solid var(--border); flex-shrink: 0; background: var(--surface); }
  .search-input-wrap { position: relative; flex: 1; }
  .search-icon { position: absolute; left: 11px; top: 50%; transform: translateY(-50%); color: var(--text-muted); pointer-events: none; }
  .search-input { width: 100%; background: var(--surface2); border: 1px solid var(--border); border-radius: var(--radius-sm); color: var(--text); padding: 9px 12px 9px 36px; font-size: 13px; font-family: inherit; outline: none; transition: all var(--transition); box-shadow: inset 0 1px 3px rgba(0,0,0,0.1); }
  .search-input:focus { border-color: var(--accent); box-shadow: inset 0 1px 3px rgba(0,0,0,0.1), 0 0 0 2px rgba(var(--accent-rgb),0.15); }
  .filter-select { min-width: 130px; }
  .view-toggle { display: flex; gap: 2px; background: var(--surface2); border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 2px; flex-shrink: 0; }
  .view-btn { display: flex; align-items: center; justify-content: center; width: 30px; height: 30px; border-radius: calc(var(--radius-sm) - 1px); color: var(--text-muted); transition: all var(--transition); }
  .view-btn:hover { color: var(--text); background: var(--surface); }
  .view-btn.active { background: var(--surface); color: var(--accent); box-shadow: 0 1px 3px rgba(0,0,0,0.15); }

  /* ── Results Grid ── */
  .discover-body { flex: 1; overflow-y: auto; padding: 20px 24px; }
  .results-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 14px; margin-bottom: 24px; }
  .result-card { display: flex; flex-direction: column; overflow: visible; position: relative; cursor: pointer; }
  .skeleton { height: 280px; animation: shimmer 1.5s ease-in-out infinite; }
  @keyframes shimmer { 0%,100%{opacity:0.45}50%{opacity:0.75} }

  /* Card banner */
  .card-banner { height: 128px; overflow: hidden; background: var(--surface2); border-radius: var(--radius) var(--radius) 0 0; position: relative; display: flex; align-items: center; justify-content: center; flex-shrink: 0; }
  .card-banner-img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .card-banner-fallback { width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; }
  .banner-fallback-icon { width: 64px; height: 64px; object-fit: contain; border-radius: 12px; opacity: 0.75; }
  .banner-letter { font-size: 40px; font-weight: 800; color: var(--border); }
  .card-icon-overlay { position: absolute; top: calc(128px - 26px); left: 12px; width: 52px; height: 52px; border-radius: 10px; border: 2px solid var(--surface); object-fit: contain; image-rendering: pixelated; z-index: 3; background: var(--surface2); box-shadow: 0 2px 8px rgba(0,0,0,0.3); }

  /* Card body */
  .card-body { padding: 12px; display: flex; flex-direction: column; gap: 7px; flex: 1; }
  .card-body.has-icon { padding-top: 34px; }
  .card-title-wrap { display: flex; flex-direction: column; gap: 1px; min-width: 0; flex: 1; }
  .card-title { font-size: 13px; font-weight: 600; color: var(--text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .card-author { font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .card-desc { font-size: 12px; line-height: 1.4; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .card-tags { display: flex; flex-wrap: wrap; gap: 4px; }
  .card-footer { display: flex; align-items: center; justify-content: space-between; margin-top: auto; padding-top: 4px; }
  .dl-count { display: flex; align-items: center; gap: 4px; font-size: 11px; }
  .install-btn { flex-shrink: 0; }

  /* Pagination */
  .pagination { display: flex; align-items: center; justify-content: center; gap: 16px; padding: 16px 0; }
  .page-info { font-size: 13px; }
  .empty-state { padding: 64px; text-align: center; font-size: 14px; display: flex; flex-direction: column; align-items: center; gap: 12px; }
  .btn-spinner { width: 10px; height: 10px; border: 1.5px solid rgba(255,255,255,0.4); border-top-color: white; border-radius: 50%; animation: spin 0.7s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg) } }

  /* ── Compact list ── */
  .results-list { display: flex; flex-direction: column; gap: 5px; margin-bottom: 24px; }
  .compact-row { display: flex; align-items: center; gap: 12px; padding: 10px 14px; cursor: pointer; box-shadow: 0 1px 3px rgba(0,0,0,0.1); }
  .compact-row-skeleton { height: 68px; animation: shimmer 1.5s ease-in-out infinite; }
  .compact-icon { width: 40px; height: 40px; border-radius: 8px; overflow: hidden; flex-shrink: 0; border: 1px solid var(--border); background: var(--surface2); display: flex; align-items: center; justify-content: center; }
  .compact-icon-img { width: 100%; height: 100%; object-fit: contain; image-rendering: pixelated; }
  .compact-icon-placeholder { font-size: 18px; font-weight: 800; color: var(--text-muted); }
  .compact-info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
  .compact-title-row { display: flex; align-items: baseline; min-width: 0; overflow: hidden; white-space: nowrap; }
  .compact-desc { font-size: 12px; line-height: 1.4; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .compact-tags { display: flex; flex-wrap: nowrap; gap: 4px; overflow: hidden; }
  .compact-actions { display: flex; flex-direction: column; align-items: flex-end; gap: 6px; flex-shrink: 0; }

  /* ── Detail Overlay ── */
  .detail-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.7); z-index: 200; display: flex; align-items: center; justify-content: center; padding: 16px; }
  .detail-panel { width: 100%; max-width: 1000px; max-height: 90vh; background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); display: flex; flex-direction: column; overflow: hidden; }

  /* Detail header */
  .detail-header { display: flex; align-items: center; gap: 10px; padding: 10px 14px; border-bottom: 1px solid var(--border); flex-shrink: 0; background: linear-gradient(to bottom, color-mix(in srgb, var(--accent) 5%, var(--surface)), var(--surface)); }
  .detail-header-actions { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }
  .detail-icon { width: 36px; height: 36px; object-fit: contain; image-rendering: pixelated; flex-shrink: 0; display: block; border-radius: 8px; border: 1px solid var(--border); }
  .detail-icon-placeholder { width: 36px; height: 36px; border-radius: 8px; border: 1px solid var(--border); background: var(--surface2); display: flex; align-items: center; justify-content: center; font-size: 15px; font-weight: 800; color: var(--text-muted); flex-shrink: 0; }
  .detail-title-block { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .detail-title { font-size: 13px; font-weight: 700; color: var(--text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .detail-author { font-size: 11px; }
  .detail-stat-row { display: flex; gap: 5px; flex-shrink: 0; }
  .detail-stat-chip { display: inline-flex; align-items: center; gap: 4px; padding: 3px 8px; border-radius: 100px; font-size: 11px; font-weight: 500; background: var(--surface3); border: 1px solid var(--border); color: var(--text-muted); }
  .detail-spinner { width: 18px; height: 18px; border: 2px solid var(--border); border-top-color: var(--accent); border-radius: 50%; animation: spin 0.7s linear infinite; }

  /* Detail tabs */
  .detail-tabs { display: flex; gap: 2px; padding: 0 20px; border-bottom: 1px solid var(--border); flex-shrink: 0; }
  .detail-tab { display: flex; align-items: center; gap: 6px; padding: 10px 14px; font-size: 13px; color: var(--text-muted); border-bottom: 2px solid transparent; margin-bottom: -1px; transition: all var(--transition); white-space: nowrap; }
  .detail-tab:hover { color: var(--text); }
  .detail-tab.active { color: var(--accent); border-bottom-color: var(--accent); }
  .tab-badge { display: inline-flex; align-items: center; justify-content: center; min-width: 18px; height: 16px; padding: 0 5px; background: var(--surface3); border-radius: 8px; font-size: 10px; font-weight: 600; color: var(--text-muted); }
  .detail-tab.active .tab-badge { background: rgba(var(--accent-rgb),0.15); color: var(--accent); }

  /* Detail body */
  .detail-body { flex: 1; overflow: hidden; display: flex; flex-direction: column; }
  .tab-pane { flex: 1; overflow-y: auto; padding: 20px; display: flex; flex-direction: column; gap: 16px; }

  /* Overview */
  .detail-short-desc { font-size: 14px; color: var(--text-dim); line-height: 1.6; }
  .overview-section { display: flex; flex-direction: column; gap: 10px; }
  .overview-tags { display: flex; flex-wrap: wrap; gap: 6px; }
  .detail-links { display: flex; flex-wrap: wrap; gap: 8px; }
  .detail-link { display: inline-flex; align-items: center; gap: 5px; font-size: 12px; color: var(--accent); border: 1px solid rgba(var(--accent-rgb),0.25); border-radius: var(--radius-sm); padding: 5px 10px; transition: all var(--transition); }
  .detail-link:hover { background: rgba(var(--accent-rgb),0.1); border-color: var(--accent); }

  /* Markdown body */
  .md-body { font-size: 13px; line-height: 1.7; color: var(--text-dim); }
  .md-body :global(.md-h2) { font-size: 16px; font-weight: 700; color: var(--text); margin: 16px 0 8px; border-bottom: 1px solid var(--border); padding-bottom: 6px; }
  .md-body :global(.md-h3) { font-size: 14px; font-weight: 600; color: var(--text); margin: 14px 0 6px; }
  .md-body :global(.md-h4) { font-size: 13px; font-weight: 600; color: var(--text); margin: 12px 0 4px; }
  .md-body :global(.md-p) { margin: 0 0 10px; }
  .md-body :global(.md-list) { margin: 6px 0 10px 18px; display: flex; flex-direction: column; gap: 3px; }
  .md-body :global(.md-list li) { list-style: disc; }
  .md-body :global(.md-quote) { border-left: 3px solid var(--accent); padding: 6px 12px; background: rgba(var(--accent-rgb),0.06); border-radius: 0 var(--radius-sm) var(--radius-sm) 0; color: var(--text-muted); margin: 8px 0; font-style: italic; }
  .md-body :global(.md-hr) { border: none; border-top: 1px solid var(--border); margin: 14px 0; }
  .md-body :global(.md-code) { background: var(--surface3); border: 1px solid var(--border); border-radius: 3px; padding: 1px 5px; font-family: monospace; font-size: 12px; color: var(--accent); }
  .md-body :global(.md-code-block) { background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 12px 14px; overflow-x: auto; font-family: monospace; font-size: 12px; color: var(--text-dim); margin: 8px 0; white-space: pre; }
  .md-body :global(.md-link) { color: var(--accent); text-decoration: underline; text-underline-offset: 2px; cursor: pointer; }
  .md-body :global(.md-badge) { display: inline-flex; align-items: center; padding: 3px 10px; border-radius: 100px; font-size: 11px; font-weight: 600; background: var(--surface3); border: 1px solid var(--border); color: var(--text-dim); cursor: pointer; text-decoration: none; transition: all var(--transition); margin: 2px; }
  .md-body :global(.md-badge:hover) { background: rgba(var(--accent-rgb),0.12); border-color: rgba(var(--accent-rgb),0.35); color: var(--accent); }
  .md-body :global(strong) { color: var(--text); font-weight: 600; }
  .md-body :global(em) { font-style: italic; }
  .md-body :global(del) { text-decoration: line-through; opacity: 0.7; }
  .md-body :global(.md-img) { max-width: 100%; height: auto; border-radius: var(--radius-sm); margin: 4px 0; display: inline-block; vertical-align: middle; }
  .md-body :global(img) { max-width: 100%; height: auto; border-radius: var(--radius-sm); display: block; margin: 8px 0; }
  .md-body :global(center) { text-align: center; display: block; }
  .md-body :global(sup) { font-size: 10px; vertical-align: super; color: var(--text-muted); }
  .md-body :global(h1), .md-body :global(h2) { font-size: 16px; font-weight: 700; color: var(--text); margin: 16px 0 8px; border-bottom: 1px solid var(--border); padding-bottom: 6px; }
  .md-body :global(h3) { font-size: 14px; font-weight: 600; color: var(--text); margin: 14px 0 6px; }
  .md-body :global(h4) { font-size: 13px; font-weight: 600; color: var(--text); margin: 12px 0 4px; }
  .md-body :global(p) { margin: 0 0 10px; }

  /* Versions */
  .compat-context { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--text-dim); background: var(--surface2); border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 6px 10px; }
  .compat-filter-row { display: flex; align-items: center; gap: 10px; margin-bottom: 10px; }
  .compat-filter-none { font-size: 11px; }
  .link-btn { background: none; border: none; padding: 0; color: var(--accent); font-size: 13px; cursor: pointer; text-decoration: underline; }
  /* ─── Modpack Install Wizard Modal ─── */
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
    display: flex; align-items: center; justify-content: center;
    margin-bottom: 8px;
    animation: wizFloat 3s ease-in-out 1.2s infinite;
  }
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
  .wp { transform-box: fill-box; animation: wizPetalIn 0.5s cubic-bezier(0.34,1.56,0.64,1) both; }
  .wpn { transform-origin: center bottom; animation-delay: 0.00s; }
  .wpe { transform-origin: left   center; animation-delay: 0.08s; }
  .wps { transform-origin: center top;    animation-delay: 0.16s; }
  .wpw { transform-origin: right  center; animation-delay: 0.24s; }
  @keyframes wizPetalIn { 0% { opacity: 0; transform: scale(0); } 100% { opacity: 1; transform: scale(1); } }
  .wiz-inner { transform-box: fill-box; transform-origin: center; animation: wizInnerIn 0.3s ease-out 0.4s both; }
  @keyframes wizInnerIn { from { opacity: 0; transform: scale(0.15); } to { opacity: 0.9; transform: scale(1); } }
  .wiz-dot { transform-box: fill-box; transform-origin: center; animation: wizDotIn 0.25s ease-out 0.6s both; }
  @keyframes wizDotIn { from { opacity: 0; transform: scale(0); } to { opacity: 1; transform: scale(1); } }
  .wiz-star { transform-box: fill-box; transform-origin: center; animation: wizStarPulse 2.4s ease-in-out 0.8s infinite; }
  @keyframes wizStarPulse { 0%, 100% { transform: scale(1); opacity: 1; } 50% { transform: scale(0.92); opacity: 0.8; } }
  @keyframes wizFloat {
    0%, 100% { transform: translateY(0); }
    50%       { transform: translateY(-5px); }
  }
  @keyframes fadeIn  { from { opacity: 0 } to { opacity: 1 } }
  @keyframes slideIn { from { transform: translateY(-12px); opacity: 0 } to { transform: translateY(0); opacity: 1 } }
  .versions-list { display: flex; flex-direction: column; gap: 6px; }
  .version-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 12px; background: var(--surface2); border: 1px solid var(--border); border-radius: var(--radius-sm); transition: border-color var(--transition); }
  .version-row.compat-ok { border-color: rgba(52,211,153,0.2); }
  .version-row.compat-warn { border-color: rgba(251,191,36,0.2); background: rgba(251,191,36,0.03); }
  .version-left { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 5px; }
  .version-top-row { display: flex; align-items: center; gap: 7px; flex-wrap: wrap; }
  .version-number { font-size: 13px; font-weight: 600; color: var(--text); font-family: monospace; }
  .version-name { font-size: 11px; }
  .version-type-badge { font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.05em; padding: 2px 6px; border-radius: 100px; border: 1px solid; }
  .compat-badge { font-size: 10px; font-weight: 600; padding: 2px 7px; border-radius: 100px; }
  .compat-ok-badge { color: var(--success); background: rgba(52,211,153,0.1); border: 1px solid rgba(52,211,153,0.25); }
  .compat-warn-badge { color: var(--warning, #f59e0b); background: rgba(251,191,36,0.1); border: 1px solid rgba(251,191,36,0.25); }
  .version-tags-row { display: flex; flex-wrap: wrap; gap: 4px; }
  .ver-tag { font-size: 10px; background: var(--surface3); border: 1px solid var(--border); border-radius: 4px; padding: 1px 6px; color: var(--text-dim); font-family: monospace; }
  .loader-tag { display: inline-flex; align-items: center; gap: 3px; }
  .loader-icon-sm { display: flex; align-items: center; width: 11px; height: 11px; }
  .loader-icon-sm :global(svg) { width: 11px; height: 11px; }
  .ver-tag-more { font-size: 10px; color: var(--text-muted); padding: 1px 4px; }
  .version-right { display: flex; flex-direction: column; align-items: flex-end; gap: 4px; flex-shrink: 0; }
  .btn-warn { background: rgba(251,191,36,0.15) !important; border-color: rgba(251,191,36,0.4) !important; color: var(--warning, #f59e0b) !important; }
  .btn-sm { padding: 5px 10px; font-size: 12px; display: inline-flex; align-items: center; gap: 5px; }

  /* Gallery tab */
  .gallery-pane { gap: 12px; }
  .gallery-main { position: relative; border-radius: var(--radius); overflow: hidden; background: var(--surface2); flex-shrink: 0; }
  .gallery-img { width: 100%; max-height: 380px; object-fit: contain; display: block; background: var(--bg); }
  .gallery-caption { position: absolute; bottom: 0; left: 0; right: 0; background: rgba(0,0,0,0.6); color: white; font-size: 12px; padding: 6px 12px; }
  .gallery-nav { position: absolute; top: 50%; transform: translateY(-50%); background: rgba(0,0,0,0.6); border: none; border-radius: 50%; width: 32px; height: 32px; display: flex; align-items: center; justify-content: center; color: white; cursor: pointer; transition: background var(--transition); z-index: 2; }
  .gallery-nav:hover:not(:disabled) { background: rgba(0,0,0,0.85); }
  .gallery-nav:disabled { opacity: 0.3; cursor: default; }
  .gallery-prev { left: 8px; }
  .gallery-next { right: 8px; }
  .gallery-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(100px, 1fr)); gap: 6px; }
  .gallery-thumb { border-radius: var(--radius-sm); overflow: hidden; border: 2px solid transparent; cursor: pointer; padding: 0; transition: border-color var(--transition); aspect-ratio: 16/9; }
  .gallery-thumb.active { border-color: var(--accent); }
  .gallery-thumb-img { width: 100%; height: 100%; object-fit: cover; display: block; }

  /* ── Instance Picker ── */
  .picker-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.6); z-index: 300; display: flex; align-items: center; justify-content: center; padding: 16px; }
  .picker-dialog { width: 360px; max-width: 90vw; padding: 20px; display: flex; flex-direction: column; gap: 14px; max-height: 80vh; }
  .picker-header { display: flex; align-items: center; justify-content: space-between; }
  .picker-title { font-size: 15px; font-weight: 700; }
  .picker-item-name { font-size: 12px; padding: 6px 10px; background: var(--surface2); border-radius: var(--radius-sm); border: 1px solid var(--border); }
  .picker-list { display: flex; flex-direction: column; gap: 4px; overflow-y: auto; max-height: 280px; }
  .picker-inst { display: flex; align-items: center; gap: 10px; padding: 9px 10px; border-radius: var(--radius-sm); cursor: pointer; border: 1px solid transparent; transition: all var(--transition); }
  .picker-inst:hover { background: var(--surface2); }
  .picker-inst.selected { background: rgba(var(--accent-rgb),0.08); border-color: rgba(var(--accent-rgb),0.3); }
  .picker-inst-dot { width: 14px; height: 14px; border-radius: 50%; border: 2px solid var(--border); flex-shrink: 0; transition: all var(--transition); }
  .picker-inst-dot.active { border-color: var(--accent); background: var(--accent); }
  .picker-inst-info { display: flex; flex-direction: column; gap: 1px; }
  .picker-inst-name { font-size: 13px; font-weight: 500; color: var(--text); }
  .picker-inst-meta { font-size: 11px; }
  .picker-actions { display: flex; justify-content: flex-end; gap: 8px; padding-top: 4px; border-top: 1px solid var(--border); }

  /* Toast */
  .install-toast { position: fixed; bottom: 24px; right: 24px; background: var(--success); color: #fff; padding: 10px 18px; border-radius: var(--radius-sm); font-size: 13px; font-weight: 500; box-shadow: 0 4px 16px rgba(0,0,0,0.3); z-index: 400; animation: slideUp 0.2s ease; }
  .toast-error { background: var(--error) !important; }
  @keyframes slideUp { from { opacity:0; transform:translateY(8px) } to { opacity:1; transform:translateY(0) } }

  /* Icon btn */
  .icon-btn { width: 28px; height: 28px; border-radius: var(--radius-sm); display: flex; align-items: center; justify-content: center; color: var(--text-muted); background: none; border: none; cursor: pointer; transition: all var(--transition); }
  .icon-btn:hover { background: var(--surface2); color: var(--text); }

  /* Badges */
  .badge { display: inline-flex; align-items: center; padding: 2px 7px; border-radius: 100px; font-size: 10px; font-weight: 600; }
  .badge-muted { background: var(--surface3); color: var(--text-muted); border: 1px solid var(--border); }
  .badge-info { background: rgba(96,165,250,0.12); color: #60a5fa; border: 1px solid rgba(96,165,250,0.25); }
</style>
