<script>
  import { RefreshCw, Send } from '@lucide/svelte';
  export let time = "00:00:00";
  export let lastEvent = "No recent events";
  export let onSend = () => {};
  export let nodesCount = 0;
</script>

<div class="shell-top">
  <div style="display:flex;align-items:center;gap:10px">
    <span style="color:var(--signal-300);font-family:var(--font-mono);font-size:10px;letter-spacing:0.22em">● LIVE</span>
    <span class="stamp" style="font-size:10px">MESH.TOPO / N={nodesCount} / 2.4GHz</span>
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
