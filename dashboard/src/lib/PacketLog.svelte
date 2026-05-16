<script>
  export let entries = [];
  export let dense = false;

  function colorForState(state) {
    if (state === "ok") return "var(--signal-300)";
    if (state === "relay") return "var(--uplink-300)";
    if (state === "lost") return "var(--lost-300)";
    return "var(--bone-100)";
  }
</script>

<div style="flex:1;display:flex;flex-direction:column;padding:{dense ? '0' : '20px 24px'};overflow:hidden">
  {#if !dense}
    <div style="margin-bottom:14px">
      <div class="stamp" style="font-size:10px">04 / PACKET LOG</div>
      <div style="font-family:var(--font-display);font-size:24px;letter-spacing:-0.01em;color:var(--bone-100);margin-top:4px">
        {entries.length} packets
      </div>
    </div>
  {/if}
  <div style="flex:1;overflow:auto;background:var(--bg-panel);border:1px solid var(--border);border-radius:2px">
    <div style="display:grid;grid-template-columns:90px 60px 70px 1fr 70px;padding:8px 14px;background:var(--ink-200);color:var(--bone-400);letter-spacing:0.14em;font-family:var(--font-mono);font-size:9px;text-transform:uppercase;border-bottom:1px solid var(--border);position:sticky;top:0;z-index:1">
      <div>UTC</div><div>NODE</div><div>KIND</div><div>PAYLOAD</div><div style="text-align:right">STATE</div>
    </div>
    {#each entries as e (e.id)}
      <div style="display:grid;grid-template-columns:90px 60px 70px 1fr 70px;padding:7px 14px;font-family:var(--font-mono);font-size:11px;border-bottom:1px solid var(--ink-200);align-items:center">
        <div style="color:var(--bone-500)">{e.time}</div>
        <div style="color:{colorForState(e.nodeState || 'ok')}">{e.node}</div>
        <div style="color:var(--bone-100)">{e.kind}</div>
        <div style="color:var(--bone-300);white-space:nowrap;overflow:hidden;text-overflow:ellipsis" title={e.payload}>{e.payload}</div>
        <div style="text-align:right;color:{colorForState(e.result === 'OK' ? 'ok' : e.result === 'RELAY' ? 'relay' : 'lost')}">{e.result}</div>
      </div>
    {/each}
  </div>
</div>
