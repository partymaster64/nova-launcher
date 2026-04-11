<script>
  import { createEventDispatcher, tick } from 'svelte'

  // options: string[] | {value, label}[]
  export let value = undefined
  export let options = []
  export let placeholder = 'Auswählen'
  export let disabled = false
  export let searchable = false

  const dispatch = createEventDispatcher()

  let open = false
  let wrapEl
  let triggerEl
  let searchEl
  let search = ''

  // Position for fixed dropdown
  let dropTop = 0
  let dropLeft = 0
  let dropWidth = 0
  let dropUp = false

  $: selected = options.find(o => (typeof o === 'string' ? o : o.value) === value)
  $: selectedLabel = selected
    ? (typeof selected === 'string' ? selected : selected.label)
    : placeholder
  $: isPlaceholder = value === undefined || value === '' || value === null

  $: filtered = search
    ? options.filter(o => {
        const l = typeof o === 'string' ? o : o.label
        return l.toLowerCase().includes(search.toLowerCase())
      })
    : options

  async function toggle() {
    if (disabled) return
    open = !open
    if (open) {
      await tick()
      positionDropdown()
      if (searchable && searchEl) searchEl.focus()
    } else {
      search = ''
    }
  }

  function positionDropdown() {
    if (!triggerEl) return
    const rect = triggerEl.getBoundingClientRect()
    const spaceBelow = window.innerHeight - rect.bottom
    const dropH = Math.min(240, filtered.length * 33 + (searchable ? 40 : 8))
    dropUp = spaceBelow < dropH + 8 && rect.top > dropH
    dropLeft = rect.left
    dropWidth = rect.width
    dropTop = dropUp ? rect.top - dropH - 4 : rect.bottom + 4
  }

  function pick(opt) {
    value = typeof opt === 'string' ? opt : opt.value
    open = false
    search = ''
    dispatch('change', value)
  }

  function onWindowClick(e) {
    if (wrapEl && !wrapEl.contains(e.target)) { open = false; search = '' }
  }

  function onKey(e) {
    if (e.key === 'Escape') { open = false; search = '' }
  }
</script>

<svelte:window on:mousedown={onWindowClick} on:keydown={onKey} on:scroll={() => open && positionDropdown()} />

<div class="sel" class:open class:disabled bind:this={wrapEl}>
  <button class="sel-trigger" on:click={toggle} type="button" {disabled} bind:this={triggerEl}>
    <span class="sel-value" class:muted={isPlaceholder}>{selectedLabel}</span>
    <svg class="sel-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="12" height="12">
      <polyline points="6 9 12 15 18 9"/>
    </svg>
  </button>
</div>

{#if open}
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="sel-dropdown"
    style="top:{dropTop}px;left:{dropLeft}px;width:{dropWidth}px"
    on:mousedown|stopPropagation
    role="listbox"
    tabindex="-1"
  >
    {#if searchable}
      <div class="sel-search-wrap">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="12" height="12"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
        <input class="sel-search" type="text" placeholder="Suchen…" bind:value={search} bind:this={searchEl} />
      </div>
    {/if}
    <div class="sel-list">
      {#each filtered as opt}
        {@const v = typeof opt === 'string' ? opt : opt.value}
        {@const l = typeof opt === 'string' ? opt : opt.label}
        <button class="sel-item" class:active={v === value} on:click={() => pick(opt)} type="button">{l}</button>
      {/each}
      {#if filtered.length === 0}
        <div class="sel-empty">Keine Ergebnisse</div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .sel { position: relative; width: 100%; }

  .sel-trigger {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 0 10px;
    height: 34px;
    background: var(--surface2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-size: 13px;
    color: var(--text);
    cursor: pointer;
    text-align: left;
    transition: border-color 0.15s;
    font-family: inherit;
  }
  .sel-trigger:hover:not(:disabled) { border-color: color-mix(in srgb, var(--accent) 60%, transparent); }
  .open .sel-trigger { border-color: var(--accent); box-shadow: 0 0 0 2px rgba(var(--accent-rgb),0.12); }
  .disabled .sel-trigger { opacity: 0.5; cursor: not-allowed; }

  .sel-value { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sel-value.muted { color: var(--text-muted); }

  .sel-chevron {
    flex-shrink: 0;
    color: var(--text-muted);
    transition: transform 0.15s;
  }
  .open .sel-chevron { transform: rotate(180deg); color: var(--accent); }

  /* Fixed-position dropdown — renders above all modals */
  .sel-dropdown {
    position: fixed;
    z-index: 9999;
    background: var(--surface2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    box-shadow: 0 8px 32px rgba(0,0,0,0.55);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .sel-search-wrap {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 7px 10px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    color: var(--text-muted);
  }
  .sel-search {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    font-size: 12.5px;
    color: var(--text);
    font-family: inherit;
  }
  .sel-search::placeholder { color: var(--text-muted); }

  .sel-list {
    overflow-y: auto;
    max-height: 220px;
    padding: 4px;
    scrollbar-color: var(--border) var(--surface2);
    scrollbar-width: thin;
  }

  .sel-item {
    display: block;
    width: 100%;
    padding: 7px 10px;
    font-size: 13px;
    color: var(--text-dim);
    text-align: left;
    border-radius: calc(var(--radius-sm) - 1px);
    cursor: pointer;
    background: none;
    border: none;
    transition: background 0.1s, color 0.1s;
    font-family: inherit;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sel-item:hover { background: var(--surface3); color: var(--text); }
  .sel-item.active { color: var(--accent); background: rgba(var(--accent-rgb),0.1); font-weight: 500; }

  .sel-empty { font-size: 12px; color: var(--text-muted); padding: 10px 12px; text-align: center; }
</style>
