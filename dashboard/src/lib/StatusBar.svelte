<script>
  import { RefreshCw, Send, Radio } from '@lucide/svelte';
  import RaptorCanvas from '../RaptorCanvas.svelte';
  export let time = "00:00:00";
  export let lastEvent = "No recent events";
  export let onSend = () => {};
  export let nodesCount = 0;
  export let connected = true;
  export let radio = null;

  let showHopPicker = false;
  const hopChannels = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];

  async function doHop(ch) {
    showHopPicker = false;
    await fetch('/api/channel/hop', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ channel: ch }),
    });
  }

  function toggleHopPicker() {
    showHopPicker = !showHopPicker;
  }
</script>

<div class="shell-top" style="height: var(--header-h)">
  <!-- Left Area -->
  <div style="display:flex;align-items:center;gap:12px">
    {#if connected}
      <span style="color: var(--signal-300); font-family: var(--font-mono); font-size: 10px; letter-spacing: 0.22em; flex-shrink: 0">
        ● LIVE
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
        <span class="stamp" style="font-size:10px">N={nodesCount}</span>
      {/if}
    {:else}
      <span style="color: var(--lost-300); font-family: var(--font-mono); font-size: 10px; letter-spacing: 0.22em; flex-shrink: 0">
        ● OFFLINE
      </span>
    {/if}
  </div>

  <!-- Middle Area (Event log or layout spacer) -->
  {#if connected}
    <div style="flex:1;display:flex;align-items:center;gap:10px;min-width:0;overflow:hidden;white-space:nowrap">
      <div class="stamp" style="font-size:10px;color:var(--bone-400);flex-shrink:0">LAST EVENT</div>
      <div style="font-family:var(--font-mono);font-size:11px;color:var(--bone-200);overflow:hidden;text-overflow:ellipsis">{lastEvent}</div>
    </div>
  {:else}
    <div style="flex:1"></div>
  {/if}
  
  <div style="flex-shrink: 0; padding: 0 16px;">
    <RaptorCanvas width={120} height={24} />
  </div>

  <!-- Right Area -->
  <div style="display:flex;align-items:center;gap:14px">
    <div class="stamp" style="font-size:10px;color:var(--bone-300)">UTC {time}</div>
    <div style="width:1px;height:18px;background:var(--border)"></div>
    <button class="icon-btn" title="Refresh"><RefreshCw size={14} /></button>
    {#if connected}
      <div style="position:relative">
        <button class="btn" on:click={toggleHopPicker} title="Coordinate channel hop across all mesh nodes"
          style="border-color: rgba(255,179,71,0.4); color: var(--uplink-300); gap:5px">
          <Radio size={13} /><span>HOP</span>
        </button>
        {#if showHopPicker}
          <div style="
            position:absolute; top:2rem; right:0;
            background:var(--surface-2,#1a1d22); border:1px solid var(--border);
            border-radius:6px; padding:6px; display:flex; flex-wrap:wrap;
            gap:4px; width:160px; z-index:100;
            box-shadow: 0 4px 16px rgba(0,0,0,0.5)
          ">
            <div style="width:100%;font-family:var(--font-mono);font-size:9px;color:var(--bone-400);letter-spacing:0.12em;margin-bottom:2px;padding:0 2px">
              SELECT CHANNEL
            </div>
            {#each hopChannels as ch}
              <button
                on:click={() => doHop(ch)}
                style="
                  flex:0 0 calc(33.33% - 3px); padding:4px 0; font-size:10px;
                  font-family:var(--font-mono); cursor:pointer; border-radius:4px;
                  border: 1px solid {radio && radio.channel === ch ? 'rgba(111,195,255,0.6)' : 'var(--border)'};
                  background: {radio && radio.channel === ch ? 'rgba(111,195,255,0.12)' : 'transparent'};
                  color: {radio && radio.channel === ch ? 'var(--drift-300)' : 'var(--bone-300)'};
                "
              >CH {ch}</button>
            {/each}
          </div>
        {/if}
      </div>
      <button class="btn btn-primary" on:click={onSend}><Send size={14} /><span>SEND</span></button>
    {/if}
  </div>
</div>
