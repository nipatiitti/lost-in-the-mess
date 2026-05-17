<script>
  import { onMount, onDestroy } from "svelte";
  import PacketLog from "./PacketLog.svelte";

  export let activeStreams = [];
  export let nodes = [];
  export let entries = [];
  export let lastResult = null;
  export let onSend = () => {};

  let text = "";
  let target = nodes.length > 1 ? nodes.find((n) => n.id !== "HUB")?.id : "";

  $: targetNode = nodes.find((n) => n.id === target);
  $: expectedPrr = targetNode ? targetNode.prr : 1;
  $: willLikelyLand = expectedPrr >= 0.6;
  $: chatEntries = entries.filter(e => !(e.kind === "text" && /^\[-?\d+(\.\d+)?,-?\d+(\.\d+)?\]/.test(e.payload)));

  // Send progress state
  let ws = null;
  let currentSendId = null;
  let sendProgress = 0;   // 0.0–1.0
  let sendStatus = null;  // null | 'transmitting' | 'done' | 'failed'
  let sendStatusTimer = null;

  onMount(() => {
    const proto = location.protocol === "https:" ? "wss" : "ws";
    ws = new WebSocket(`${proto}://${location.host}/api/raptor/ws`);
    ws.onmessage = (e) => {
      let ev;
      try { ev = JSON.parse(e.data); } catch { return; }
      if (currentSendId === null) return;
      if (ev.type === "SenderProgress" && ev.data.id === currentSendId) {
        sendProgress = ev.data.packets_sent / ev.data.target;
      } else if (ev.type === "SenderComplete" && ev.data.id === currentSendId) {
        sendProgress = 1;
        sendStatus = "done";
        scheduleReset(3000);
      } else if (ev.type === "SenderFailed" && ev.data.id === currentSendId) {
        sendStatus = "failed";
        scheduleReset(4000);
      }
    };
  });

  function scheduleReset(ms) {
    clearTimeout(sendStatusTimer);
    sendStatusTimer = setTimeout(() => {
      currentSendId = null;
      sendProgress = 0;
      sendStatus = null;
    }, ms);
  }

  async function sendTextMsg() {
    if (!text.trim()) return;
    clearTimeout(sendStatusTimer);
    currentSendId = null;
    sendProgress = 0;
    sendStatus = "transmitting";

    const result = await onSend({
      target,
      kind: "text",
      text,
      image: null,
    });

    if (result?.id != null) {
      currentSendId = result.id;
    } else {
      sendStatus = null;
    }
    text = "";
  }

  function handleKeydown(e) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      sendTextMsg();
    }
  }

  onDestroy(() => {
    ws?.close();
    clearTimeout(sendStatusTimer);
  });

  function getLabel(id) {
    const n = nodes.find(n => n.id === `N-${id.toString().padStart(2, '0')}`);
    return n ? n.label : `N-${id.toString().padStart(2, '0')}`;
  }
</script>

<div style="padding:20px 24px;overflow:auto;flex:1;display:grid;grid-template-columns:1fr 360px;gap:20px">
  <div>
    <div class="stamp" style="font-size: 10px; margin-bottom: 4px;">04 / STREAMS</div>
    <div style="font-family: var(--font-display); font-size: 24px; letter-spacing: -0.01em; color: var(--bone-100); margin-bottom: 24px;">
      Live Mesh Video Feeds
    </div>

    {#if activeStreams.length === 0}
      <div style="border: 1px dashed var(--border-strong); border-radius: 2px; padding: 48px; text-align: center; color: var(--bone-400); background: var(--ink-100);">
        <div style="font-family: var(--font-mono); font-size: 12px; letter-spacing: 0.14em; text-transform: uppercase;">
          NO ACTIVE STREAMS
        </div>
        <div class="stamp" style="font-size: 9px; margin-top: 8px;">
          START A STREAM FROM THE UPLINK TAB ON ANY NODE
        </div>
      </div>
    {:else}
      <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 20px;">
        {#each activeStreams as streamId}
          <div style="border: 1px solid var(--border); background: #000; border-radius: 2px; overflow: hidden; position: relative;">
            <!-- Header overlay -->
            <div style="
              position: absolute; top: 12px; left: 12px; z-index: 10;
              background: rgba(0,0,0,0.6); backdrop-filter: blur(4px);
              padding: 6px 12px; border-radius: 2px;
              display: flex; align-items: center; gap: 8px;
            ">
              <span style="width: 8px; height: 8px; border-radius: 50%; background: var(--lost-300); animation: pulse-dot 1s ease infinite;"></span>
              <span style="font-family: var(--font-mono); font-size: 11px; letter-spacing: 0.1em; color: var(--bone-100);">
                {getLabel(streamId)}
              </span>
            </div>
            
            <!-- MJPEG Stream -->
            <!-- svelte-ignore a11y-missing-attribute -->
            <img 
              src="/api/video/stream/{streamId}" 
              style="width: 100%; display: block; object-fit: contain; background: #111;"
              on:error={(e) => { e.target.style.display = 'none'; e.target.nextElementSibling.style.display = 'flex'; }}
              on:load={(e) => { e.target.style.display = 'block'; e.target.nextElementSibling.style.display = 'none'; }}
            />
            
            <!-- Fallback when loading/failed -->
            <div style="display: none; position: absolute; inset: 0; align-items: center; justify-content: center; background: #111; color: var(--bone-400); font-family: var(--font-mono); font-size: 11px;">
              CONNECTING...
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <div style="display:flex;flex-direction:column;gap:14px">
    <div
      class="panel bracketed"
      style="flex:1; display:flex; flex-direction:column; min-height:0; padding: 16px;"
    >
      <div class="h-eyebrow" style="margin-bottom:12px">MESH CHAT</div>
      <div style="flex:1; min-height:0; display:flex; flex-direction:column">
        <PacketLog entries={chatEntries} dense={true} />
      </div>
      
      <div style="margin-top: 12px;">
        <input
          type="text"
          bind:value={text}
          on:keydown={handleKeydown}
          placeholder="Type message while streaming..."
          style="
            width:100%;background:var(--ink-100);border:1px solid var(--border);
            border-radius:2px;padding:10px 14px;color:var(--bone-100);
            font-family:var(--font-mono);font-size:13px;outline:none;
            box-sizing:border-box;
          "
        />
        <div style="display:flex; justify-content:space-between; align-items:center; margin-top:6px;">
          <div class="stamp" style="font-size:9px; opacity:0.6;">
            PRESS ↵ TO TRANSMIT
          </div>
          {#if sendStatus}
            <div style="width:100px; height:3px; background:var(--border); border-radius:1px; overflow:hidden">
              <div style="
                height:100%;
                width:{sendStatus === 'done' ? 100 : Math.round(sendProgress * 100)}%;
                background:{sendStatus === 'failed' ? 'var(--lost-300)' : 'var(--signal-300)'};
                transition:width 0.3s ease;
              "></div>
            </div>
          {:else if lastResult}
            <div
              class="mono"
              style="font-size:11px;color: {lastResult.result === 'OK'
                ? 'var(--signal-300)'
                : lastResult.result === 'RELAY'
                  ? 'var(--uplink-300)'
                  : 'var(--lost-300)'}"
            >
              ▸ {lastResult.message}
            </div>
          {/if}
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  @keyframes pulse-dot {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }
</style>
