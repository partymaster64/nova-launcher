<script>
  import { invoke } from '@tauri-apps/api/core'
  import { onMount } from 'svelte'
  import { config, accounts, currentPage, detailInstanceId, instanceUpdates, crashEvent, isOnline } from '../../store.js'
  import CreateInstanceWizard from '../components/CreateInstanceWizard.svelte'
  import InstanceCard from '../components/InstanceCard.svelte'
  import LaunchOverlay from '../components/LaunchOverlay.svelte'
  import { t } from '../../lib/i18n.js'

  let cfg = null
  let accs = []
  let updMap = {}
  let showWizard = false

  config.subscribe(v => (cfg = v))
  accounts.subscribe(v => (accs = v))
  instanceUpdates.subscribe(v => (updMap = v))

  $: activeAccount = accs.find(a => a.uuid === cfg?.active_account_uuid)

  let runningIds = []
  let launchingId = null

  // Clear LaunchOverlay if crash modal appears
  $: if ($crashEvent) launchingId = null

  onMount(() => {
    const tick = async () => {
      try {
        const r = await invoke('get_running_instances')
        runningIds = r
        if (launchingId && r.includes(launchingId)) launchingId = null
      } catch (_) {}
    }
    tick()
    const iv = setInterval(tick, 750)
    return () => clearInterval(iv)
  })

  async function launchInstance(id) {
    if (!activeAccount) return
    launchingId = id
    try {
      await invoke('launch_instance', { instanceId: id })
    } catch (e) {
      console.error('Launch error:', e)
      launchingId = null
    }
  }

  async function stopInstance(id) {
    try { await invoke('kill_instance', { instanceId: id }) } catch (e) { console.error(e) }
  }

  function openDetail(id) {
    detailInstanceId.set(id)
    currentPage.set('instance-detail')
  }

  async function onWizardComplete(event) {
    showWizard = false
    config.set(event.detail)
  }

</script>

<div class="page">
  <div class="page-header">
    <h1 class="page-title">{$t('instances.title')}</h1>
    <button class="btn btn-primary" on:click={() => (showWizard = true)} disabled={!$isOnline} title={!$isOnline ? 'Kein Internet' : ''}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
      {$t('instances.newInstance')}
    </button>
  </div>

  <div class="page-body">
    {#if !cfg?.instances?.length}
      <div class="empty-state">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="48" height="48" style="color:var(--text-muted)"><rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>
        <h3>{$t('instances.empty')}</h3>
        <p class="text-muted">{$t('instances.emptyDesc')}</p>
        <button class="btn btn-primary" on:click={() => (showWizard = true)}>{$t('instances.createFirst')}</button>
      </div>
    {:else}
      <div class="instance-list">
        {#each cfg.instances as inst (inst.id)}
          <div class="card-wrap">
            <InstanceCard
              {inst}
              {activeAccount}
              launchState={runningIds.includes(inst.id) ? { type: 'running' } : null}
              isLaunching={launchingId === inst.id}
              updateCount={updMap[inst.id]?.length ?? 0}
              on:detail={() => openDetail(inst.id)}
              on:play={() => launchInstance(inst.id)}
              on:stop={() => stopInstance(inst.id)}
            />
          </div>
        {/each}

        <button class="new-instance-btn" on:click={() => (showWizard = true)} disabled={!$isOnline} title={!$isOnline ? 'Kein Internet' : ''}>
          {#if !$isOnline}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="22" height="22"><line x1="1" y1="1" x2="23" y2="23"/><path d="M16.72 11.06A10.94 10.94 0 0 1 19 12.55M5 12.55a10.94 10.94 0 0 1 5.17-2.39M10.71 5.05A16 16 0 0 1 22.56 9M1.42 9a15.91 15.91 0 0 1 4.7-2.88M8.53 16.11a6 6 0 0 1 6.95 0M12 20h.01"/></svg>
            <span>Kein Internet</span>
          {:else}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="22" height="22"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
            <span>{$t('instances.newInstance')}</span>
          {/if}
        </button>
      </div>
    {/if}
  </div>
</div>

{#if showWizard}
  <CreateInstanceWizard on:close={() => (showWizard = false)} on:complete={onWizardComplete} />
{/if}

{#if launchingId}
  {@const launchingInst = cfg?.instances?.find(i => i.id === launchingId)}
  <LaunchOverlay instanceName={launchingInst?.name ?? ''} />
{/if}

<style>
  .empty-state {
    display: flex; flex-direction: column; align-items: center; gap: 14px;
    padding: 80px 24px; text-align: center;
    background: linear-gradient(135deg, color-mix(in srgb, var(--accent) 4%, var(--surface)), var(--surface));
    border: 1px solid var(--border); border-radius: var(--radius-lg);
    margin: 0 auto; max-width: 480px;
  }
  .empty-state > svg {
    width: 56px; height: 56px; padding: 14px;
    background: rgba(var(--accent-rgb),0.1); border: 1px solid rgba(var(--accent-rgb),0.2);
    border-radius: 50%; color: var(--accent) !important;
  }
  .empty-state h3 { font-size: 18px; font-weight: 700; }

.instance-list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 8px;
  }

  .card-wrap { display: flex; flex-direction: column; }

  .new-instance-btn {
    display: flex; align-items: center; justify-content: center; gap: 8px;
    padding: 14px;
    background: none;
    border: 1.5px dashed var(--border);
    border-radius: var(--radius);
    color: var(--text-muted);
    font-size: 13px; font-weight: 500;
    cursor: pointer;
    transition: all var(--transition);
    grid-column: 1 / -1;
  }
  .new-instance-btn:hover { color: var(--accent); border-color: var(--accent); background: rgba(var(--accent-rgb),0.05); }
</style>
