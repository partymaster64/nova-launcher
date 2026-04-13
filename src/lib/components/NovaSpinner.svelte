<script>
  export let size = 56   // px
  export let label = ''
</script>

<div class="spinner-wrap">
  <svg
    viewBox="0 0 100 100"
    xmlns="http://www.w3.org/2000/svg"
    width={size}
    height={size}
  >
    <defs>
      <linearGradient id="ns-grad" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0%"   stop-color="color-mix(in srgb, var(--accent) 55%, white)"/>
        <stop offset="100%" stop-color="var(--accent)"/>
      </linearGradient>
      <radialGradient id="ns-glow" cx="50%" cy="50%" r="50%">
        <stop offset="0%"   stop-color="var(--accent)" stop-opacity="0.4"/>
        <stop offset="100%" stop-color="var(--accent)" stop-opacity="0"/>
      </radialGradient>
      <filter id="ns-blur">
        <feGaussianBlur stdDeviation="5"/>
      </filter>
    </defs>

    <!-- Glow backdrop -->
    <circle class="glow" cx="50" cy="50" r="44"
      fill="url(#ns-glow)" filter="url(#ns-blur)"/>

    <!-- Spinning star group -->
    <g class="star">
      <!-- 4 petals shooting from center -->
      <path class="petal pn" d="M50,50 Q40,40 50,7 Q60,40 50,50Z"  fill="url(#ns-grad)"/>
      <path class="petal pe" d="M50,50 Q60,40 93,50 Q60,60 50,50Z" fill="url(#ns-grad)"/>
      <path class="petal ps" d="M50,50 Q60,60 50,93 Q40,60 50,50Z" fill="url(#ns-grad)"/>
      <path class="petal pw" d="M50,50 Q40,60 7,50 Q40,40 50,50Z"  fill="url(#ns-grad)"/>
      <!-- Inner highlight -->
      <path class="inner" d="M50,27 Q55,45 70,50 Q55,55 50,73 Q45,55 30,50 Q45,45 50,27Z"
        fill="white" opacity="0.88"/>
      <!-- Center dot -->
      <circle class="dot" cx="50" cy="50" r="4" fill="white"/>
    </g>
  </svg>
  {#if label}
    <span class="spinner-label">{label}</span>
  {/if}
</div>

<style>
  .spinner-wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }

  /* ── Build-in animation ── */
  .petal {
    transform-box: fill-box;
    animation: petalIn 0.45s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  .pn { transform-origin: center bottom; animation-delay: 0.00s; }
  .pe { transform-origin: left   center; animation-delay: 0.06s; }
  .ps { transform-origin: center top;    animation-delay: 0.12s; }
  .pw { transform-origin: right  center; animation-delay: 0.18s; }

  @keyframes petalIn {
    0%   { opacity: 0; transform: scale(0); }
    100% { opacity: 1; transform: scale(1); }
  }

  .inner {
    transform-box: fill-box;
    transform-origin: center;
    animation: innerIn 0.35s ease-out 0.4s both;
  }
  @keyframes innerIn {
    from { opacity: 0; transform: scale(0.2); }
    to   { opacity: 0.88; transform: scale(1); }
  }

  .dot {
    transform-box: fill-box;
    transform-origin: center;
    animation: dotIn 0.25s ease-out 0.6s both;
  }
  @keyframes dotIn {
    from { opacity: 0; transform: scale(0); }
    to   { opacity: 1; transform: scale(1); }
  }

  /* ── Continuous pulse + slow rotation ── */
  .star {
    transform-box: fill-box;
    transform-origin: center;
    animation: starPulse 2.2s ease-in-out 0.8s infinite;
  }
  @keyframes starPulse {
    0%   { transform: rotate(0deg)   scale(1);    opacity: 1; }
    50%  { transform: rotate(45deg)  scale(0.85); opacity: 0.7; }
    100% { transform: rotate(0deg)   scale(1);    opacity: 1; }
  }

  .glow {
    animation: glowPulse 2.2s ease-in-out 0.8s infinite;
  }
  @keyframes glowPulse {
    0%,100% { opacity: 0.6; transform: scale(1); }
    50%     { opacity: 1;   transform: scale(1.2); }
  }

  .spinner-label {
    font-size: 12px;
    color: var(--text-muted);
    letter-spacing: 0.02em;
  }
</style>
