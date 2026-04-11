<script>
  import { createEventDispatcher } from 'svelte'

  export let checked = false
  export let disabled = false

  const dispatch = createEventDispatcher()

  function toggle() {
    if (disabled) return
    checked = !checked
    dispatch('change', checked)
  }
</script>

<!-- svelte-ignore a11y-click-events-have-key-events -->
<!-- svelte-ignore a11y-no-static-element-interactions -->
<span class="chk-wrap" class:disabled on:click={toggle}>
  <span class="chk-box" class:checked role="checkbox" aria-checked={checked}>
    {#if checked}
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="10" height="10">
        <polyline points="20 6 9 17 4 12"/>
      </svg>
    {/if}
  </span>
  <slot />
</span>

<style>
  .chk-wrap {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    cursor: pointer;
    user-select: none;
    font-size: 13px;
    color: var(--text-dim);
  }
  .chk-wrap.disabled { opacity: 0.5; cursor: not-allowed; }

  .chk-box {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    border: 1.5px solid var(--border);
    border-radius: 4px;
    background: var(--surface2);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.15s, border-color 0.15s;
    color: #fff;
  }
  .chk-wrap:hover:not(.disabled) .chk-box:not(.checked) {
    border-color: var(--accent);
  }
  .chk-box.checked {
    background: var(--accent);
    border-color: var(--accent);
  }
</style>
