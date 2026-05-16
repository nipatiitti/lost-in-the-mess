<script>
  import { onMount } from 'svelte';
  export let nodes = [];
  export let selected = null;
  export let setSelected = () => {};

  let svgWidth = 800;
  let svgHeight = 480;
  let svgContainer;

  $: byId = Object.fromEntries(nodes.map(n => [n.id, n]));
  
  $: pos = (n) => ({ x: (n.x / 100) * svgWidth, y: (n.y / 100) * svgHeight });

  $: links = nodes.filter(n => n.id !== "HUB").map(n => ["HUB", n.id, n.prr]);

  function colorForState(state) {
    if (state === "ok") return "var(--signal-300)";
    if (state === "relay") return "var(--uplink-300)";
    if (state === "lost") return "var(--lost-300)";
    return "var(--bone-100)";
  }

  function colorForPrr(prr) {
    if (prr >= 0.76) return "var(--signal-300)";
    if (prr >= 0.26) return "var(--uplink-300)";
    return "var(--lost-300)";
  }

  onMount(() => {
    const ro = new ResizeObserver(entries => {
      for (const e of entries) {
        svgWidth = e.contentRect.width;
        svgHeight = e.contentRect.height;
      }
    });
    if (svgContainer) {
      ro.observe(svgContainer);
    }
    return () => ro.disconnect();
  });
</script>

<div bind:this={svgContainer} style="position:relative;flex:1;min-height:0;overflow:hidden;background:var(--ink-050)">
  <!-- grid background -->
  <div class="grid-overlay" style="position:absolute;inset:0"></div>
  <!-- scanlines -->
  <div class="scanlines" style="position:absolute;inset:0;pointer-events:none"></div>

  <!-- corner brackets -->
  <div style="position:absolute;width:14px;height:14px;border-color:var(--bone-100);border-style:solid;opacity:0.5;top:8px;left:8px;border-width:1px 0 0 1px"></div>
  <div style="position:absolute;width:14px;height:14px;border-color:var(--bone-100);border-style:solid;opacity:0.5;top:8px;right:8px;border-width:1px 1px 0 0"></div>
  <div style="position:absolute;width:14px;height:14px;border-color:var(--bone-100);border-style:solid;opacity:0.5;bottom:8px;left:8px;border-width:0 0 1px 1px"></div>
  <div style="position:absolute;width:14px;height:14px;border-color:var(--bone-100);border-style:solid;opacity:0.5;bottom:8px;right:8px;border-width:0 1px 1px 0"></div>

  <!-- badge top-left -->
  <div style="position:absolute;top:16px;left:24px;z-index:2">
    <div class="stamp" style="font-size:10px;color:var(--bone-300)">02 / TOPOLOGY</div>
    <div style="font-family:var(--font-display);font-size:24px;letter-spacing:-0.01em;color:var(--bone-100);margin-top:4px">{nodes.length} nodes in range</div>
  </div>

  <!-- legend bottom-right -->
  <div style="position:absolute;bottom:16px;right:24px;display:flex;gap:8px;z-index:2">
    <div class="chip chip-ok"><div class="chip-dot"></div>PRR ≥ 0.75</div>
    <div class="chip chip-warm"><div class="chip-dot"></div>0.25 – 0.75</div>
    <div class="chip chip-lost"><div class="chip-dot"></div>&lt; 0.25</div>
  </div>

  <svg width={svgWidth} height={svgHeight} style="position:absolute;inset:0">
    <defs>
      <radialGradient id="halo" cx="50%" cy="50%" r="50%">
        <stop offset="0%" stop-color="#6DF7B5" stop-opacity="0.18"></stop>
        <stop offset="100%" stop-color="#6DF7B5" stop-opacity="0"></stop>
      </radialGradient>
    </defs>
    
    {#each nodes.filter(n => n.id === "HUB") as n}
      {@const p = pos(n)}
      <circle cx={p.x} cy={p.y} r={Math.min(svgWidth, svgHeight) * 0.45} fill="url(#halo)"></circle>
    {/each}

    <!-- Links -->
    {#each links as [a, b, q]}
      {@const A = byId[a]}
      {@const B = byId[b]}
      {#if A && B}
        {@const pa = pos(A)}
        {@const pb = pos(B)}
        {@const color = colorForPrr(q)}
        {@const isLost = q < 0.25}
        {@const isRelay = q >= 0.25 && q < 0.75}
        <line x1={pa.x} y1={pa.y} x2={pb.x} y2={pb.y}
          stroke={color}
          stroke-opacity={isLost ? 0.55 : isRelay ? 0.65 : 0.8}
          stroke-width={isLost ? 1 : 1.2}
          stroke-dasharray={isLost ? "2 4" : isRelay ? "4 3" : "0"}
          stroke-linecap="round"></line>
      {/if}
    {/each}

    <!-- Nodes -->
    {#each nodes as n (n.id)}
      {@const p = pos(n)}
      {@const color = colorForState(n.state)}
      {@const isHub = n.id === "HUB"}
      {@const isSel = selected === n.id}
      {@const isLive = n.state === "ok"}
      <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
      <g class={isLive ? "node-live" : ""} style="cursor:pointer" on:click={() => setSelected(n.id)}>
        {#if isHub}
          <circle cx={p.x} cy={p.y} r="22" fill="none" stroke="var(--signal-300)" stroke-opacity="0.3"></circle>
        {/if}
        {#if isLive}
          <circle class="halo" cx={p.x} cy={p.y} r={isHub ? 18 : 12} fill={color} fill-opacity="0.18"></circle>
        {/if}
        <circle cx={p.x} cy={p.y} r={isHub ? 12 : 6} fill="var(--ink-050)" stroke={color} stroke-width={isSel ? 2 : 1.5}></circle>
        <circle cx={p.x} cy={p.y} r={isHub ? 6 : 3} fill={color}></circle>
        {#if isSel}
          <circle cx={p.x} cy={p.y} r={isHub ? 22 : 14} fill="none" stroke={color} stroke-dasharray="3 3" stroke-opacity="0.7"></circle>
        {/if}
        <text x={p.x + (isHub ? 16 : 10)} y={p.y + 4} font-family="JetBrains Mono, monospace" font-size="10" letter-spacing="0.14em" fill={color}>{n.label}</text>
        <text x={p.x + (isHub ? 16 : 10)} y={p.y + 16} font-family="JetBrains Mono, monospace" font-size="9" fill="var(--bone-400)">{n.prr.toFixed(2)}</text>
      </g>
    {/each}
  </svg>
</div>
