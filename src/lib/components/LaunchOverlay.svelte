<script>
  import { fade } from 'svelte/transition'
  export let instanceName = ''
  export let subtitle = ''
</script>

<div class="overlay" transition:fade={{ duration: 300 }}>
  <div class="content">
    <div class="logo-wrap">
      <svg viewBox="0 0 160 160" xmlns="http://www.w3.org/2000/svg"
        style="
          --ni-1: color-mix(in srgb, var(--accent) 18%, black);
          --ni-2: color-mix(in srgb, var(--accent) 32%, black);
          --ni-3: color-mix(in srgb, var(--accent) 52%, black);
          --ni-4: var(--accent);
          --ni-5: color-mix(in srgb, var(--accent) 60%, white);
          --ni-6: color-mix(in srgb, var(--accent) 35%, white);
          --ni-7: color-mix(in srgb, var(--accent) 14%, white);
        "
      >
        <!-- Builds from center outward: r7 (innermost) first, r1 (outermost) last -->
        <rect class="r r1" x="4"  y="4"  width="152" height="152" rx="18" fill="var(--ni-1)"/>
        <rect class="r r2" x="16" y="16" width="128" height="128" rx="16" fill="var(--ni-2)"/>
        <rect class="r r3" x="28" y="28" width="104" height="104" rx="14" fill="var(--ni-3)"/>
        <rect class="r r4" x="40" y="40" width="80"  height="80"  rx="12" fill="var(--ni-4)"/>
        <rect class="r r5" x="52" y="52" width="56"  height="56"  rx="10" fill="var(--ni-5)"/>
        <rect class="r r6" x="64" y="64" width="32"  height="32"  rx="7"  fill="var(--ni-6)"/>
        <rect class="r r7" x="73" y="73" width="14"  height="14"  rx="4"  fill="var(--ni-7)"/>
      </svg>
    </div>

    <p class="name">{instanceName || 'Nova Launcher'}</p>
    <p class="sub">{subtitle}</p>
    <div class="dots">
      <span class="dot d1"></span>
      <span class="dot d2"></span>
      <span class="dot d3"></span>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 300;
    background: var(--overlay-bg, rgba(0, 0, 0, 0.82));
    backdrop-filter: var(--overlay-blur, blur(12px));
    -webkit-backdrop-filter: var(--overlay-blur, blur(12px));
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0;
    animation: contentIn 0.4s ease-out both;
  }

  @keyframes contentIn {
    from { opacity: 0; transform: translateY(12px); }
    to   { opacity: 1; transform: translateY(0); }
  }

  /* Logo */
  .logo-wrap {
    width: 140px;
    height: 140px;
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 28px;
    animation: float 3s ease-in-out 1.5s infinite;
  }

  .logo-wrap svg {
    width: 140px;
    height: 140px;
  }

  @keyframes float {
    0%, 100% { transform: translateY(0); }
    50%       { transform: translateY(-6px); }
  }

  /* Each rect builds up — inside to outside */
  .r {
    transform-box: fill-box;
    transform-origin: center;
    animation: buildRect 0.5s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }

  /* r7 = innermost → first; r1 = outermost → last */
  .r7 { animation-delay: 0.00s; }
  .r6 { animation-delay: 0.12s; }
  .r5 { animation-delay: 0.24s; }
  .r4 { animation-delay: 0.36s; }
  .r3 { animation-delay: 0.48s; }
  .r2 { animation-delay: 0.60s; }
  .r1 { animation-delay: 0.72s; }

  @keyframes buildRect {
    0%   { opacity: 0; transform: scale(0.2); }
    70%  { opacity: 1; transform: scale(1.08); }
    100% { opacity: 1; transform: scale(1); }
  }

  /* Text */
  .name {
    font-size: 22px;
    font-weight: 700;
    color: var(--text);
    letter-spacing: 0.01em;
    animation: fadeUp 0.4s ease-out 1.0s both;
  }
  .sub {
    font-size: 13px;
    color: var(--text-muted);
    margin-top: 4px;
    animation: fadeUp 0.4s ease-out 1.1s both;
  }

  @keyframes fadeUp {
    from { opacity: 0; transform: translateY(6px); }
    to   { opacity: 1; transform: translateY(0); }
  }

  /* Animated dots */
  .dots {
    display: flex;
    gap: 6px;
    margin-top: 20px;
    animation: fadeUp 0.4s ease-out 1.2s both;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    animation: dotPulse 1.2s ease-in-out 1.4s infinite;
  }
  .d2 { animation-delay: 1.6s; }
  .d3 { animation-delay: 1.8s; }

  @keyframes dotPulse {
    0%, 80%, 100% { opacity: 0.25; transform: scale(0.8); }
    40%           { opacity: 1;    transform: scale(1.1); }
  }
</style>
