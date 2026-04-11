<script>
  import { createEventDispatcher } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { loaderIcon, loaderColor, loaderLabel } from '../loaderIcons.js'
  import { t } from '../i18n.js'

  export let inst
  export let activeAccount = null
  export let launchState   = null   // { type, step, percent, error } | null
  export let isLaunching   = false  // true while waiting for process to start
  export let updateCount   = 0      // number of pending updates

  const dispatch = createEventDispatcher()

  $: isRunning = launchState?.type === 'running'
  $: isDl      = launchState?.type === 'downloading'
  $: lColor    = loaderColor(inst.loader)
  $: lIcon     = loaderIcon(inst.loader)
  $: lText     = inst.loader_version
    ? `${loaderLabel(inst.loader)} ${inst.loader_version}`
    : loaderLabel(inst.loader)

  let iconSrc = null
  $: if (inst.icon_path) {
    invoke('read_icon', { path: inst.icon_path })
      .then(url => { iconSrc = url })
      .catch(() => { iconSrc = null })
  } else {
    iconSrc = null
  }

  function onCardClick(e) {
    if (e.target.closest('button')) return
    dispatch('detail')
  }
</script>

<!-- svelte-ignore a11y-click-events-have-key-events -->
<!-- svelte-ignore a11y-no-static-element-interactions -->
<div
  class="icard"
  class:is-running={isRunning}
  class:is-dl={isDl}
  on:click={onCardClick}
>
  <!-- Left: instance icon -->
  <div class="icard-icon" style="--lc: {lColor}">
    {#if iconSrc}
      <img src={iconSrc} alt="" class="icard-icon-img" />
      <div class="icard-icon-fb" style="display:none">{inst.name[0]?.toUpperCase() ?? '?'}</div>
    {:else}
      <div class="icard-icon-fb">{inst.name[0]?.toUpperCase() ?? '?'}</div>
    {/if}
  </div>

  <!-- Center: name + meta -->
  <div class="icard-info">
    <div class="icard-name-row">
      <div class="icard-name">{inst.name}</div>
      {#if updateCount > 0}
        <span class="icard-upd-dot" title="{updateCount} Update{updateCount !== 1 ? 's' : ''} verfügbar">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="10" height="10"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 .49-3.5"/></svg>
        </span>
      {/if}
    </div>
    <div class="icard-sub" style="color:{lColor}">
      {#if lIcon}<span class="icard-licon">{@html lIcon}</span>{/if}
      <span>{lText}</span>
      <span class="dot">·</span>
      <span class="ver">{inst.version || '—'}</span>
    </div>

    {#if isDl}
      <div class="icard-prog">
        <div class="prog-track"><div class="prog-fill" style="width:{(launchState.percent||0)*100}%"></div></div>
        <span class="prog-lbl">{launchState.step || $t('instanceCard.loading')}</span>
      </div>
    {:else if launchState?.type === 'error'}
      <div class="icard-err">{launchState.error}</div>
    {/if}
  </div>

  <!-- Right: play / stop / loading -->
  <div class="icard-btn-wrap">
    {#if isRunning}
      <button class="btn-stop" title={$t('common.stop')} on:click|stopPropagation={() => dispatch('stop')}>
        <svg viewBox="0 0 24 24" fill="currentColor" width="14" height="14">
          <rect x="4" y="4" width="16" height="16" rx="2"/>
        </svg>
      </button>
    {:else if isLaunching || isDl}
      <div class="btn-play btn-play-loading" title={$t('instanceCard.starting')}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="15" height="15" class="spin">
          <path d="M21 12a9 9 0 1 1-6.219-8.56"/>
        </svg>
      </div>
    {:else}
      <button
        class="btn-play"
        title={$t('common.play')}
        disabled={!inst.version || !activeAccount}
        on:click|stopPropagation={() => dispatch('play')}
      >
        <svg viewBox="0 0 24 24" fill="currentColor" width="16" height="16">
          <polygon points="5 3 19 12 5 21 5 3"/>
        </svg>
      </button>
    {/if}
  </div>
</div>

<style>
  .icard {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    transition: all var(--transition);
    user-select: none;
    min-width: 0;
    box-shadow: 0 1px 4px rgba(0,0,0,0.15);
  }
  .icard:hover {
    border-color: rgba(var(--accent-rgb),0.4);
    background: var(--surface2);
    box-shadow: 0 4px 16px rgba(0,0,0,0.25), 0 0 0 1px rgba(var(--accent-rgb),0.15);
    transform: translateY(-1px);
  }
  .icard.is-running {
    border-color: rgba(52,211,153,0.6);
    box-shadow: 0 0 0 1px rgba(52,211,153,0.2), 0 4px 20px rgba(52,211,153,0.12);
  }
  .icard.is-dl {
    border-color: rgba(var(--accent-rgb),0.6);
    box-shadow: 0 0 0 1px rgba(var(--accent-rgb),0.15), 0 4px 16px rgba(0,0,0,0.2);
  }

  /* Instance icon */
  .icard-icon {
    width: 44px; height: 44px;
    border-radius: 8px;
    border: 1px solid color-mix(in srgb, var(--lc) 40%, transparent);
    background: color-mix(in srgb, var(--lc) 15%, transparent);
    display: flex; align-items: center; justify-content: center;
    flex-shrink: 0; overflow: hidden;
  }
  .icard-icon-img {
    width: 100%; height: 100%;
    object-fit: cover; image-rendering: pixelated; display: block;
  }
  .icard-icon-fb {
    width: 100%; height: 100%;
    display: flex; align-items: center; justify-content: center;
    font-size: 18px; font-weight: 800;
    color: var(--lc);
  }

  /* Info */
  .icard-info {
    flex: 1; min-width: 0;
    display: flex; flex-direction: column; gap: 3px;
  }
  .icard-name-row { display: flex; align-items: center; gap: 6px; min-width: 0; }
  .icard-name {
    font-size: 14px; font-weight: 600; color: var(--text);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .icard-upd-dot {
    display: inline-flex; align-items: center; justify-content: center; flex-shrink: 0;
    width: 18px; height: 18px; border-radius: 50%;
    background: rgba(251,191,36,0.1); border: 1px solid rgba(251,191,36,0.25);
    color: rgba(251,191,36,0.7);
  }
  .icard-sub {
    font-size: 11px; font-weight: 500;
    display: flex; align-items: center; gap: 4px; flex-wrap: nowrap;
  }
  .icard-licon { display: flex; align-items: center; width: 11px; height: 11px; flex-shrink: 0; }
  .icard-licon :global(svg) { width: 11px; height: 11px; }
  .dot { color: var(--text-muted); }
  .ver { color: var(--text-muted); font-weight: 400; }

  .icard-prog { display: flex; flex-direction: column; gap: 3px; margin-top: 2px; }
  .prog-track { height: 3px; background: var(--surface3); border-radius: 2px; overflow: hidden; }
  .prog-fill  { height: 100%; background: var(--accent); border-radius: 2px; transition: width 0.3s ease; }
  .prog-lbl   { font-size: 10px; color: var(--text-muted); }

  .icard-err { font-size: 10px; color: var(--error); margin-top: 2px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  /* Button */
  .icard-btn-wrap { flex-shrink: 0; }

  .btn-play, .btn-stop {
    width: 36px; height: 36px;
    border-radius: 50%;
    display: flex; align-items: center; justify-content: center;
    cursor: pointer; transition: all var(--transition);
    border: none;
  }
  .btn-play {
    background: var(--accent); color: #fff;
    box-shadow: 0 2px 8px rgba(var(--accent-rgb),0.4);
  }
  .btn-play:hover:not(:disabled) { background: var(--accent-hover); transform: scale(1.09); box-shadow: 0 4px 14px rgba(var(--accent-rgb),0.55); }
  .btn-play:disabled { background: var(--surface3); color: var(--text-muted); cursor: not-allowed; opacity: 0.6; box-shadow: none; }
  .btn-play-loading { cursor: default; opacity: 0.75; }
  .spin { animation: spin 0.9s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg) } }

  .btn-stop {
    background: rgba(248,113,113,0.15);
    color: var(--error);
    border: 1px solid rgba(248,113,113,0.35) !important;
  }
  .btn-stop:hover { background: rgba(248,113,113,0.28); transform: scale(1.07); }


</style>
