<script>
  export let nodes = [];
  export let selected = null;
  export let setSelected = () => {};

  let filter = "all";

  $: counts = {
    all: nodes.length,
    ok: nodes.filter(n => n.state === "ok").length,
    relay: nodes.filter(n => n.state === "relay").length,
    lost: nodes.filter(n => n.state === "lost").length,
  };

  $: filtered = filter === "all" ? nodes : nodes.filter(n => n.state === filter);

  function setFilter(f) {
    filter = f;
  }

  function colorForState(state) {
    if (state === "ok") return "var(--signal-300)";
    if (state === "relay") return "var(--uplink-300)";
    if (state === "lost") return "var(--lost-300)";
    return "var(--bone-100)";
  }

  function labelForState(state) {
    if (state === "ok") return "NOMINAL";
    if (state === "relay") return "RELAY";
    if (state === "lost") return "LOST";
    return "IDLE";
  }

  function chipToneForState(state) {
    if (state === "ok") return "ok";
    if (state === "relay") return "warm";
    if (state === "lost") return "lost";
    return "idle";
  }
</script>

<div style="padding:20px 24px;overflow:auto;flex:1">
  <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:18px">
    <div>
      <div class="stamp" style="font-size:10px">01 / NODES</div>
      <div style="font-family:var(--font-display);font-size:24px;letter-spacing:-0.01em;color:var(--bone-100);margin-top:4px">
        {filtered.length} nodes — {filter === "all" ? "all states" : `state: ${filter}`}
      </div>
    </div>
    <div style="display:flex;gap:6px">
      <button class="filter-pill {filter === 'all' ? 'active base-idle' : ''}" on:click={() => setFilter('all')}>All · {counts.all}</button>
      <button class="filter-pill {filter === 'ok' ? 'active base-ok' : ''}" on:click={() => setFilter('ok')}>Nominal · {counts.ok}</button>
      <button class="filter-pill {filter === 'relay' ? 'active base-warm' : ''}" on:click={() => setFilter('relay')}>Relay · {counts.relay}</button>
      <button class="filter-pill {filter === 'lost' ? 'active base-lost' : ''}" on:click={() => setFilter('lost')}>Lost · {counts.lost}</button>
    </div>
  </div>

  <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:12px">
    {#each filtered as node (node.id)}
      <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
      <div 
        on:click={() => setSelected(node.id)}
        class="panel"
        style="
          position:relative;
          border: {selected === node.id ? `1px solid ${colorForState(node.state)}` : '1px solid var(--border)'};
          border-radius:4px;
          padding:14px;
          cursor:pointer;
          box-shadow: {node.state === 'lost' ? '0 0 0 1px rgba(255,74,139,0.20), 0 0 12px rgba(255,74,139,0.18)' : 'none'};
          transition:border-color 120ms var(--ease-snap);
        ">
        <div style="display:flex;justify-content:space-between;align-items:flex-start;margin-bottom:10px">
          <div>
            <div style="font-family:var(--font-mono);font-size:18px;color:var(--bone-100)">{node.label}</div>
            <div class="stamp" style="font-size:9px;margin-top:2px">QUAD · 2.4GHz</div>
          </div>
          <div class="chip chip-{chipToneForState(node.state)}"><div class="chip-dot"></div>{labelForState(node.state)}</div>
        </div>
        <div style="display:flex;justify-content:space-between;align-items:baseline">
          <div class="ticker" style="font-size:28px;color:{colorForState(node.state)};font-weight:500">{node.prr.toFixed(2)}</div>
          <div class="stamp" style="font-size:9px">PRR</div>
        </div>
        <div style="margin-top:8px;height:3px;background:var(--ink-200);border-radius:1px;overflow:hidden">
          <div style="width:{node.prr*100}%;height:100%;background:{colorForState(node.state)}"></div>
        </div>
        <div class="mono" style="font-size:10px;color:var(--bone-300);margin-top:10px;display:flex;justify-content:space-between">
          <span>{node.dbm || -65}dBm</span>
          <span>{node.hops === 0 ? "HUB" : `${node.hops || 1} HOP${node.hops>1?"S":""}`}</span>
        </div>
      </div>
    {/each}
  </div>
</div>

<style>
  .filter-pill {
    padding: 5px 10px;
    background: transparent;
    color: var(--bone-300);
    border: 1px solid var(--border);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    border-radius: 999px;
    cursor: pointer;
  }
  .filter-pill.active.base-idle { color: var(--bone-100); border-color: var(--bone-100); }
  .filter-pill.active.base-ok { background: rgba(109,247,181,0.08); color: var(--signal-300); border-color: var(--signal-300); }
  .filter-pill.active.base-warm { background: rgba(255,179,71,0.08); color: var(--uplink-300); border-color: var(--uplink-300); }
  .filter-pill.active.base-lost { background: rgba(255,74,139,0.08); color: var(--lost-300); border-color: var(--lost-300); }
</style>
