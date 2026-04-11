<script>
  export let size = 56   // px
  export let label = ''
</script>

<div class="spinner-wrap">
  <svg
    viewBox="0 0 160 160"
    xmlns="http://www.w3.org/2000/svg"
    width={size}
    height={size}
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
    <rect class="r r1" x="4"  y="4"  width="152" height="152" rx="18" fill="var(--ni-1)"/>
    <rect class="r r2" x="16" y="16" width="128" height="128" rx="16" fill="var(--ni-2)"/>
    <rect class="r r3" x="28" y="28" width="104" height="104" rx="14" fill="var(--ni-3)"/>
    <rect class="r r4" x="40" y="40" width="80"  height="80"  rx="12" fill="var(--ni-4)"/>
    <rect class="r r5" x="52" y="52" width="56"  height="56"  rx="10" fill="var(--ni-5)"/>
    <rect class="r r6" x="64" y="64" width="32"  height="32"  rx="7"  fill="var(--ni-6)"/>
    <rect class="r r7" x="73" y="73" width="14"  height="14"  rx="4"  fill="var(--ni-7)"/>
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

  .r {
    transform-box: fill-box;
    transform-origin: center;
    animation: buildRect 0.45s cubic-bezier(0.34, 1.56, 0.64, 1) both,
               pulse 2.4s ease-in-out 0.9s infinite;
  }

  .r7 { animation-delay: 0.00s, 0.9s; }
  .r6 { animation-delay: 0.09s, 0.9s; }
  .r5 { animation-delay: 0.18s, 0.9s; }
  .r4 { animation-delay: 0.27s, 0.9s; }
  .r3 { animation-delay: 0.36s, 0.9s; }
  .r2 { animation-delay: 0.45s, 0.9s; }
  .r1 { animation-delay: 0.54s, 0.9s; }

  @keyframes buildRect {
    0%   { opacity: 0; transform: scale(0.2); }
    70%  { opacity: 1; transform: scale(1.08); }
    100% { opacity: 1; transform: scale(1); }
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50%       { opacity: 0.65; }
  }

  .spinner-label {
    font-size: 12px;
    color: var(--text-muted);
    letter-spacing: 0.02em;
  }
</style>
