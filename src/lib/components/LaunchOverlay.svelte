<script>
  import { fade } from 'svelte/transition'
  export let instanceName = ''
  export let subtitle = ''
</script>

<div class="overlay" transition:fade={{ duration: 300 }}>
  <div class="content">
    <div class="logo-wrap">
      <div class="glow-ring"></div>
      <svg viewBox="0 0 100 100" width="140" height="140" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <linearGradient id="lo-sg" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%"   stop-color="#f0abfc"/>
            <stop offset="50%"  stop-color="#a855f7"/>
            <stop offset="100%" stop-color="#5b21b6"/>
          </linearGradient>
        </defs>
        <g class="star-group">
          <path class="petal pn" d="M50,50 Q40,40 50,7  Q60,40 50,50Z" fill="url(#lo-sg)"/>
          <path class="petal pe" d="M50,50 Q60,40 93,50 Q60,60 50,50Z" fill="url(#lo-sg)"/>
          <path class="petal ps" d="M50,50 Q60,60 50,93 Q40,60 50,50Z" fill="url(#lo-sg)"/>
          <path class="petal pw" d="M50,50 Q40,60 7,50  Q40,40 50,50Z" fill="url(#lo-sg)"/>
          <path class="inner-star"
            d="M50,27 Q55,45 70,50 Q55,55 50,73 Q45,55 30,50 Q45,45 50,27Z"
            fill="white" opacity="0.9"/>
          <circle class="center-dot" cx="50" cy="50" r="4.5" fill="white"/>
        </g>
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
    position: relative;
    margin-bottom: 28px;
    animation: float 3s ease-in-out 1.5s infinite;
  }

  @keyframes float {
    0%, 100% { transform: translateY(0); }
    50%       { transform: translateY(-6px); }
  }

  .glow-ring {
    position: absolute;
    inset: -16px;
    border-radius: 50%;
    background: radial-gradient(circle, rgba(168,85,247,0.4) 0%, transparent 70%);
    filter: blur(14px);
    animation: glowPop 0.5s ease-out 0.6s both;
  }
  @keyframes glowPop {
    from { opacity: 0; transform: scale(0.4); }
    to   { opacity: 1; transform: scale(1); }
  }

  /* Petal shoot-in */
  .petal {
    transform-box: fill-box;
    animation: petalIn 0.5s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  .pn { transform-origin: center bottom; animation-delay: 0.10s; }
  .pe { transform-origin: left   center; animation-delay: 0.18s; }
  .ps { transform-origin: center top;    animation-delay: 0.26s; }
  .pw { transform-origin: right  center; animation-delay: 0.34s; }
  @keyframes petalIn {
    0%   { opacity: 0; transform: scale(0); }
    80%  { opacity: 1; transform: scale(1.06); }
    100% { opacity: 1; transform: scale(1); }
  }

  .inner-star {
    transform-box: fill-box;
    transform-origin: center;
    animation: innerIn 0.35s ease-out 0.55s both;
  }
  @keyframes innerIn {
    from { opacity: 0; transform: scale(0.15); }
    to   { opacity: 0.9; transform: scale(1); }
  }

  .center-dot {
    transform-box: fill-box;
    transform-origin: center;
    animation: dotIn 0.28s cubic-bezier(0.34, 1.56, 0.64, 1) 0.75s both;
  }
  @keyframes dotIn {
    from { opacity: 0; transform: scale(0); }
    to   { opacity: 1; transform: scale(1); }
  }

  /* Continuous pulse */
  .star-group {
    transform-box: fill-box;
    transform-origin: center;
    animation: starPulse 2.6s ease-in-out 1.2s infinite;
  }
  @keyframes starPulse {
    0%, 100% { transform: scale(1);    opacity: 1; }
    50%       { transform: scale(0.93); opacity: 0.8; }
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
