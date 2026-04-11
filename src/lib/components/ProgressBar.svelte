<script>
  export let percent = 0
  export let label = ''
</script>

<div class="progress-container">
  {#if label}
    <div class="progress-label">{label}</div>
  {/if}
  <div class="progress-track">
    <div
      class="progress-fill"
      style="width: {Math.min(100, Math.max(0, (percent || 0) * 100))}%"
    ></div>
  </div>
  <div class="progress-text">{Math.round((percent || 0) * 100)}%</div>
</div>

<style>
  .progress-container {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 100%;
  }

  .progress-label {
    font-size: 12px;
    color: var(--text-dim);
    text-align: center;
  }

  .progress-track {
    width: 100%;
    height: 6px;
    background: var(--surface3);
    border-radius: 100px;
    overflow: hidden;
    position: relative;
  }

  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--accent-dim), var(--accent), var(--accent-hover));
    border-radius: 100px;
    transition: width 0.2s ease;
    position: relative;
    min-width: 4px;
  }

  .progress-fill::after {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(255, 255, 255, 0.25),
      transparent
    );
    animation: shimmer 1.5s ease-in-out infinite;
  }

  @keyframes shimmer {
    0% { left: -100%; }
    100% { left: 200%; }
  }

  .progress-text {
    font-size: 11px;
    color: var(--text-muted);
    text-align: center;
  }
</style>
