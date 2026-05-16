<script>
  import { Layers, Grid, Send, Activity } from '@lucide/svelte';
  export let screen;
  export let setScreen;
  export let nodes = [];

  $: ok = nodes.filter(n => n.state === "ok").length;
  $: relay = nodes.filter(n => n.state === "relay").length;
  $: lost = nodes.filter(n => n.state === "lost").length;
  $: avgPrr = nodes.length ? (nodes.reduce((a,n) => a + n.prr, 0) / nodes.length).toFixed(2) : "0.00";

  $: items = [
    { id: "topology", label: "Topology", icon: Layers, count: nodes.length },
    { id: "nodes",    label: "Nodes",    icon: Grid, count: nodes.length },
    { id: "uplink",   label: "Uplink",   icon: Send, count: null },
    { id: "log",      label: "Log",      icon: Activity, count: "LIVE" },
  ];
</script>

<div class="shell-side">
  <!-- Brand -->
  <div style="height:var(--header-h);padding:0 16px;border-bottom:1px solid var(--border);display:flex;align-items:center;gap:10px">
    <svg width="22" height="22" viewBox="0 0 96 96">
      <circle cx="48" cy="48" r="20" fill="none" stroke="#6DF7B5" stroke-dasharray="2 3" opacity="0.6"/>
      <circle cx="48" cy="48" r="12" fill="none" stroke="#6DF7B5"/>
      <circle cx="48" cy="48" r="4" fill="#6DF7B5"/>
    </svg>
    <div style="line-height:1">
      <div style="font-family:var(--font-display);font-size:14px;font-weight:600;letter-spacing:-0.01em;color:var(--bone-100)">
        LOST IN THE MES<span style="font-family:var(--font-mono);font-size:11px;color:var(--signal-300);vertical-align:2px">(h)</span>S
      </div>
      <div class="stamp" style="font-size:9px;margin-top:4px">MESH CONSOLE · v0.4.2</div>
    </div>
  </div>

  <!-- Nav -->
  <div style="padding:10px 0 6px">
    <div class="stamp" style="padding:4px 16px 8px;font-size:9px">MESH</div>
    {#each items as item}
      <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
      <div 
        class="nav-item {screen === item.id ? 'active' : ''}"
        on:click={() => setScreen(item.id)}>
        <span style="display:inline-flex"><svelte:component this={item.icon} size={16} /></span>
        <span>{item.label}</span>
        {#if item.count !== null}
          <span class="nav-count">{item.count}</span>
        {/if}
      </div>
    {/each}
  </div>

  <!-- Mesh health summary at bottom of rail -->
  <div style="margin-top:auto;padding:14px 16px;border-top:1px solid var(--border)">
    <div class="stamp" style="font-size:9px;margin-bottom:10px">MESH HEALTH</div>
    <div style="display:flex;align-items:baseline;justify-content:space-between;margin-bottom:10px">
      <div class="ticker" style="font-size:28px;color:var(--signal-300);font-weight:500">{avgPrr}</div>
      <div class="stamp" style="font-size:9px">PRR.AVG</div>
    </div>
    <div style="display:flex;flex-direction:column;gap:6px">
      <div style="display:flex;justify-content:space-between;align-items:center;font-family:var(--font-mono);font-size:11px">
        <span style="color:var(--signal-300)">● OK</span>
        <span style="color:var(--bone-200)">{ok}</span>
      </div>
      <div style="display:flex;justify-content:space-between;align-items:center;font-family:var(--font-mono);font-size:11px">
        <span style="color:var(--uplink-300)">● RELAY</span>
        <span style="color:var(--bone-200)">{relay}</span>
      </div>
      <div style="display:flex;justify-content:space-between;align-items:center;font-family:var(--font-mono);font-size:11px">
        <span style="color:var(--lost-300)">● LOST</span>
        <span style="color:var(--bone-200)">{lost}</span>
      </div>
    </div>
  </div>
</div>
