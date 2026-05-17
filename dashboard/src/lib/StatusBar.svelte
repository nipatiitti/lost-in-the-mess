<script>
  import { RefreshCw, Send, Radio } from '@lucide/svelte';
  import RaptorCanvas from '../RaptorCanvas.svelte';
  import { onDestroy } from 'svelte';

  export let time = "00:00:00";
  export let onSend = () => {};
  export let nodesCount = 0;
  export let connected = true;
  export let radio = null;

  let showHopPicker = false;
  const hopChannels = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];

  let localRemaining = null;
  let timer = null;
  let wasPending = false;

  $: if (radio && radio.pending_channel !== null && radio.pending_channel !== undefined) {
    if (localRemaining === null || Math.abs(localRemaining - radio.remaining_seconds) > 1) {
      localRemaining = radio.remaining_seconds;
    }
    if (!timer) {
      timer = setInterval(() => {
        if (localRemaining !== null && localRemaining > 0) {
          localRemaining -= 1;
        }
      }, 1000);
    }
    showHopPicker = true;
    wasPending = true;
  } else {
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
    localRemaining = null;
    if (wasPending) {
      showHopPicker = false;
      wasPending = false;
    }
  }

  onDestroy(() => {
    if (timer) clearInterval(timer);
  });

  async function doHop(ch) {
    // Keep dialog open by not setting showHopPicker = false
    await fetch('/api/channel/hop', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ channel: ch }),
    });
  }

  function toggleHopPicker() {
    if (radio && radio.pending_channel !== null && radio.pending_channel !== undefined) {
      return; // Lock dialog open during active mesh channel hop
    }
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
  <div style="flex:1"></div>
  
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
          style="
            border-color: {radio && radio.pending_channel ? 'var(--uplink-500)' : 'rgba(255,179,71,0.4)'};
            color: var(--uplink-300);
            background: {radio && radio.pending_channel ? 'rgba(255,179,71,0.12)' : 'transparent'};
            box-shadow: {radio && radio.pending_channel ? 'var(--glow-uplink)' : 'none'};
            gap:5px;
          "
          class:hopping-active={radio && radio.pending_channel}
        >
          <Radio size={13} />
          <span>
            {#if radio && radio.pending_channel}
              HOPPING (T-{localRemaining !== null ? localRemaining : 0}s)
            {:else}
              HOP
            {/if}
          </span>
        </button>
        {#if showHopPicker}
          <div style="
            position:absolute; top:2.5rem; right:0;
            background: rgba(11,17,25,0.96);
            backdrop-filter: blur(8px);
            border:1px solid {radio && radio.pending_channel ? 'var(--uplink-500)' : 'var(--border)'};
            border-radius:4px; padding:12px; display:flex; flex-direction:column;
            gap:10px; width: {radio && radio.pending_channel ? '240px' : '170px'}; z-index:100;
            box-shadow: {radio && radio.pending_channel ? 'var(--glow-uplink)' : 'var(--shadow-2)'};
            transition: all 0.3s var(--ease-snap);
          ">
            {#if radio && radio.pending_channel !== null && radio.pending_channel !== undefined}
              <!-- Countdown View -->
              <div style="display:flex;flex-direction:column;gap:8px;font-family:var(--font-mono)">
                <div style="font-size:9px;color:var(--uplink-300);letter-spacing:0.18em;font-weight:700">
                  COORDINATED SWAP
                </div>
                
                <div style="height:1px;background:linear-gradient(90deg, rgba(255,179,71,0.5), transparent)"></div>
                
                <div style="font-size:10px;color:var(--bone-300);line-height:1.3">
                  Migrating mesh nodes to <span style="color:var(--uplink-300);font-weight:700">CH {radio.pending_channel}</span>
                </div>
                
                <!-- Large Countdown Clock -->
                <div style="
                  display:flex;align-items:baseline;justify-content:center;
                  background:rgba(255,179,71,0.06);border:1px solid rgba(255,179,71,0.15);
                  padding:10px 0;margin:4px 0;border-radius:2px;
                ">
                  <span style="font-size:11px;color:var(--uplink-300);opacity:0.6;margin-right:4px">T-MINUS</span>
                  <span style="font-size:20px;font-weight:700;color:var(--uplink-300);letter-spacing:0.05em">
                    {localRemaining !== null ? String(localRemaining).padStart(2, '0') : '00'}s
                  </span>
                </div>
                
                <!-- Progress Bar -->
                <div style="width:100%;height:4px;background:var(--ink-400);border-radius:2px;overflow:hidden">
                  <div class="progress-bar-fill" style="
                    height:100%;
                    width:{Math.max(0, Math.min(100, ((localRemaining || 0) / 60) * 100))}%;
                    background:var(--uplink-300);
                    border-radius:2px;
                    transition: width 1s linear;
                  "></div>
                </div>
                
                <!-- Status Row -->
                <div style="display:flex;align-items:center;gap:6px;margin-top:2px">
                  <div style="width:6px;height:6px;border-radius:50%;background:var(--uplink-300);box-shadow: 0 0 6px var(--uplink-300);" class="hopping-active"></div>
                  <span style="font-size:9px;color:var(--bone-400);letter-spacing:0.08em">TRANSMITTING SYNC FLOOD</span>
                </div>
              </div>
            {:else}
              <!-- Standard Channel Grid -->
              <div style="width:100%;font-family:var(--font-mono);font-size:9px;color:var(--bone-400);letter-spacing:0.12em;margin-bottom:2px;padding:0 2px">
                SELECT CHANNEL
              </div>
              <div style="display:flex;flex-wrap:wrap;gap:4px">
                {#each hopChannels as ch}
                  <button
                    on:click={() => doHop(ch)}
                    style="
                      flex:0 0 calc(33.33% - 3px); padding:4px 0; font-size:10px;
                      font-family:var(--font-mono); cursor:pointer; border-radius:2px;
                      border: 1px solid {radio && radio.channel === ch ? 'rgba(111,195,255,0.6)' : 'var(--border)'};
                      background: {radio && radio.channel === ch ? 'rgba(111,195,255,0.12)' : 'transparent'};
                      color: {radio && radio.channel === ch ? 'var(--drift-300)' : 'var(--bone-300)'};
                    "
                  >CH {ch}</button>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      </div>
      <button class="btn btn-primary" on:click={onSend}><Send size={14} /><span>SEND</span></button>
    {/if}
  </div>
</div>

<style>
  @keyframes hopPulse {
    0%, 100% { opacity: 0.7; }
    50% { opacity: 1; }
  }
  .hopping-active {
    animation: hopPulse 1.5s ease-in-out infinite;
  }
  @keyframes progressGlow {
    0%, 100% { opacity: 0.8; }
    50% { opacity: 1; }
  }
  .progress-bar-fill {
    animation: progressGlow 1.5s ease-in-out infinite;
  }
</style>
