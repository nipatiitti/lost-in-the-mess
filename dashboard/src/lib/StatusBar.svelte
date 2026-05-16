<script>
  import { RefreshCw, Send } from '@lucide/svelte';
  export let time = "00:00:00";
  export let lastEvent = "No recent events";
  export let onSend = () => {};
  export let nodesCount = 0;
  export let connected = true;
  export let radio = null;
</script>

<div class="shell-top" style="height: var(--header-h)">
  <div style="display:flex;align-items:center;gap:12px">
    <span style="color: {connected ? 'var(--signal-300)' : 'var(--lost-300)'}; font-family: var(--font-mono); font-size: 10px; letter-spacing: 0.22em; flex-shrink: 0">
      ● {connected ? 'LIVE' : 'OFFLINE'}
    </span>
    {#if radio}
      <div style="display:flex;align-items:center;gap:6px">
        <div class="tag" style="border-color: rgba(111,195,255,0.25); background: rgba(111,195,255,0.04); color: var(--drift-300); font-size: 10px; padding: 2px 6px">
          CH {radio.channel} <span style="opacity:0.6;font-size:9px;margin-left:2px">({radio.frequency_mhz} MHz)</span>
        </div>
        <div class="tag" style="border-color: var(--border); color: var(--bone-300); font-size: 10px; padding: 2px 6px">
          BW {radio.width_mhz} MHz
        </div>
        <div class="tag" style="border-color: rgba(255,179,71,0.25); background: rgba(255,179,71,0.04); color: var(--uplink-300); font-size: 10px; padding: 2px 6px">
          TX {radio.txpower_dbm.toFixed(2)} dBm
        </div>
        <div class="tag" style="border-color: rgba(109,247,181,0.25); background: rgba(109,247,181,0.04); color: var(--signal-300); font-size: 10px; padding: 2px 6px">
          PEERS: {nodesCount}
        </div>
      </div>
    {:else}
      <span class="stamp" style="font-size:10px">N={nodesCount} / OFFLINE</span>
    {/if}
  </div>
  <div style="flex:1;display:flex;align-items:center;gap:10px;min-width:0;overflow:hidden;white-space:nowrap">
    <div class="stamp" style="font-size:10px;color:var(--bone-400);flex-shrink:0">LAST EVENT</div>
    <div style="font-family:var(--font-mono);font-size:11px;color:var(--bone-200);overflow:hidden;text-overflow:ellipsis">{lastEvent}</div>
  </div>
  <div style="display:flex;align-items:center;gap:14px">
    <div class="stamp" style="font-size:10px;color:var(--bone-300)">UTC {time}</div>
    <div style="width:1px;height:18px;background:var(--border)"></div>
    <button class="icon-btn" title="Refresh"><RefreshCw size={14} /></button>
    <button class="btn btn-primary" on:click={onSend}><Send size={14} /><span>SEND</span></button>
  </div>
</div>
