<script>
  import { createEventDispatcher, onDestroy } from 'svelte'
  import { check } from '@tauri-apps/plugin-updater'
  import { relaunch } from '@tauri-apps/plugin-process'
  import { fade, fly } from 'svelte/transition'

  export let update   // the update object returned by check()

  const dispatch = createEventDispatcher()

  let phase = 'idle'   // idle | downloading | installing | done | error
  let progress = 0     // 0-100
  let downloaded = 0
  let total = 0
  let errorMsg = ''

  function fmtBytes(b) {
    if (!b) return ''
    if (b < 1024 * 1024) return `${(b / 1024).toFixed(0)} KB`
    return `${(b / 1024 / 1024).toFixed(1)} MB`
  }

  async function startUpdate() {
    phase = 'downloading'
    progress = 0
    downloaded = 0
    total = 0
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') {
          total = event.data.contentLength ?? 0
        } else if (event.event === 'Progress') {
          downloaded += event.data.chunkLength
          progress = total > 0 ? Math.round((downloaded / total) * 100) : 0
        } else if (event.event === 'Finished') {
          phase = 'installing'
          progress = 100
        }
      })
      phase = 'done'
    } catch (e) {
      phase = 'error'
      errorMsg = String(e)
    }
  }

  async function restartNow() {
    await relaunch()
  }

  // Strip common markdown from release notes for plain display
  function stripMarkdown(md) {
    return (md || '')
      .replace(/^#{1,6}\s+/gm, '')
      .replace(/\*\*(.+?)\*\*/g, '$1')
      .replace(/\*(.+?)\*/g, '$1')
      .replace(/`(.+?)`/g, '$1')
      .replace(/\[(.+?)\]\(.+?\)/g, '$1')
      .trim()
  }

  $: notes = stripMarkdown(update?.body)
</script>

<div class="backdrop" transition:fade={{ duration: 200 }}>
  <div class="modal" transition:fly={{ y: 16, duration: 250 }}>

    <!-- Logo -->
    <div class="logo-wrap">
      <div class="glow"></div>
      <svg viewBox="0 0 100 100" width="72" height="72" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <linearGradient id="um-sg" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%"   stop-color="#f0abfc"/>
            <stop offset="50%"  stop-color="#a855f7"/>
            <stop offset="100%" stop-color="#5b21b6"/>
          </linearGradient>
        </defs>
        <g class="um-star">
          <path class="um-p um-pn" d="M50,50 Q40,40 50,7  Q60,40 50,50Z" fill="url(#um-sg)"/>
          <path class="um-p um-pe" d="M50,50 Q60,40 93,50 Q60,60 50,50Z" fill="url(#um-sg)"/>
          <path class="um-p um-ps" d="M50,50 Q60,60 50,93 Q40,60 50,50Z" fill="url(#um-sg)"/>
          <path class="um-p um-pw" d="M50,50 Q40,60 7,50  Q40,40 50,50Z" fill="url(#um-sg)"/>
          <path d="M50,27 Q55,45 70,50 Q55,55 50,73 Q45,55 30,50 Q45,45 50,27Z"
                fill="white" opacity="0.9"/>
          <circle cx="50" cy="50" r="4.5" fill="white"/>
        </g>
      </svg>
    </div>

    <!-- Header -->
    <div class="header">
      <p class="label">Update verfügbar</p>
      <h2 class="version">Nova Launcher {update?.version}</h2>
    </div>

    <!-- Release notes -->
    {#if notes && phase === 'idle'}
      <div class="notes">
        <p>{notes}</p>
      </div>
    {/if}

    <!-- Progress -->
    {#if phase === 'downloading' || phase === 'installing'}
      <div class="progress-wrap">
        <div class="progress-bar">
          <div class="progress-fill" style="width:{progress}%"></div>
        </div>
        <p class="progress-label">
          {#if phase === 'installing'}
            Wird installiert…
          {:else if total > 0}
            {fmtBytes(downloaded)} / {fmtBytes(total)} ({progress}%)
          {:else}
            Wird heruntergeladen… {progress > 0 ? progress + '%' : ''}
          {/if}
        </p>
      </div>
    {/if}

    {#if phase === 'done'}
      <p class="done-msg">Update installiert. Neustart erforderlich.</p>
    {/if}

    {#if phase === 'error'}
      <p class="error-msg">{errorMsg}</p>
    {/if}

    <!-- Actions -->
    <div class="actions">
      {#if phase === 'idle'}
        <button class="btn btn-ghost" on:click={() => dispatch('close')}>Später</button>
        <button class="btn btn-primary" on:click={startUpdate}>
          Jetzt aktualisieren
        </button>
      {:else if phase === 'done'}
        <button class="btn btn-ghost" on:click={() => dispatch('close')}>Schließen</button>
        <button class="btn btn-primary" on:click={restartNow}>
          Neu starten
        </button>
      {:else if phase === 'error'}
        <button class="btn btn-ghost" on:click={() => dispatch('close')}>Schließen</button>
        <button class="btn btn-primary" on:click={startUpdate}>
          Erneut versuchen
        </button>
      {:else}
        <span class="spinner-hint">Bitte warten…</span>
      {/if}
    </div>

  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 500;
    background: rgba(0, 0, 0, 0.72);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .modal {
    background: var(--surface2);
    border: 1px solid var(--border);
    border-radius: 20px;
    padding: 32px 28px 24px;
    width: 380px;
    max-width: calc(100vw - 32px);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.6), 0 0 0 1px rgba(168,85,247,0.12);
  }

  /* ── Logo ── */
  .logo-wrap {
    position: relative;
    margin-bottom: 4px;
  }
  .glow {
    position: absolute;
    inset: -20px;
    border-radius: 50%;
    background: radial-gradient(circle, rgba(168,85,247,0.35) 0%, transparent 70%);
    filter: blur(14px);
  }
  .um-p { transform-box: fill-box; animation: umPetalIn 0.5s cubic-bezier(0.34,1.56,0.64,1) both; }
  .um-pn { transform-origin: center bottom; animation-delay: 0.00s; }
  .um-pe { transform-origin: left   center; animation-delay: 0.08s; }
  .um-ps { transform-origin: center top;    animation-delay: 0.16s; }
  .um-pw { transform-origin: right  center; animation-delay: 0.24s; }
  @keyframes umPetalIn { 0% { opacity:0; transform:scale(0); } 100% { opacity:1; transform:scale(1); } }
  .um-star { transform-box: fill-box; transform-origin: center; animation: umPulse 2.8s ease-in-out 0.6s infinite; }
  @keyframes umPulse { 0%,100% { transform:scale(1); opacity:1; } 50% { transform:scale(0.92); opacity:0.8; } }

  /* ── Header ── */
  .header { text-align: center; }
  .label {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent, #a855f7);
    margin: 0 0 6px;
  }
  .version {
    font-size: 22px;
    font-weight: 800;
    color: var(--text);
    margin: 0;
    letter-spacing: -0.02em;
  }

  /* ── Notes ── */
  .notes {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 12px 14px;
    width: 100%;
    max-height: 140px;
    overflow-y: auto;
    scrollbar-width: thin;
  }
  .notes p {
    font-size: 12px;
    color: var(--text-dim);
    line-height: 1.6;
    margin: 0;
    white-space: pre-wrap;
  }

  /* ── Progress ── */
  .progress-wrap {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .progress-bar {
    width: 100%;
    height: 6px;
    background: var(--surface);
    border-radius: 100px;
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #a855f7, #c084fc);
    border-radius: 100px;
    transition: width 0.25s ease;
  }
  .progress-label {
    font-size: 12px;
    color: var(--text-muted);
    text-align: center;
    margin: 0;
  }

  .done-msg {
    font-size: 13px;
    color: var(--success, #34d399);
    text-align: center;
    margin: 0;
  }
  .error-msg {
    font-size: 12px;
    color: var(--error, #f87171);
    text-align: center;
    margin: 0;
    word-break: break-word;
  }
  .spinner-hint {
    font-size: 12px;
    color: var(--text-muted);
  }

  /* ── Actions ── */
  .actions {
    display: flex;
    gap: 10px;
    width: 100%;
    justify-content: flex-end;
    margin-top: 4px;
  }
  .btn {
    padding: 9px 18px;
    border-radius: 8px;
    font-size: 13px;
    font-weight: 600;
    border: none;
    cursor: pointer;
    transition: opacity 0.15s, transform 0.1s;
  }
  .btn:hover { opacity: 0.88; transform: translateY(-1px); }
  .btn:active { transform: translateY(0); }
  .btn-ghost {
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text-dim);
  }
  .btn-primary {
    background: var(--accent, #a855f7);
    color: #fff;
  }
</style>
