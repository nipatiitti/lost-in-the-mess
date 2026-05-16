<script>
  import { MessageSquare, Image as ImageIcon, Video, File, Send } from '@lucide/svelte';
  import PacketLog from './PacketLog.svelte';
  export let nodes = [];
  export let entries = [];
  export let lastResult = null;
  export let onSend = () => {};

  let target = nodes.length > 1 ? nodes.find(n => n.id !== "HUB")?.id : "";
  let kind = "text";
  let text = "";
  let imageFile = null;
  let imagePreview = null;

  function handleFileSelect(e) {
    const file = e.target.files[0];
    if (!file) return;
    imageFile = file;
    const reader = new FileReader();
    reader.onload = (re) => {
      imagePreview = re.target.result;
    };
    reader.readAsDataURL(file);
  }

  $: targetNode = nodes.find(n => n.id === target);
  $: expectedPrr = targetNode ? targetNode.prr : 1;
  $: willLikelyLand = expectedPrr >= 0.6;

  function colorForState(state) {
    if (state === "ok") return "var(--signal-300)";
    if (state === "relay") return "var(--uplink-300)";
    if (state === "lost") return "var(--lost-300)";
    return "var(--bone-100)";
  }

  function rgbOfTone(tone) {
    if (tone === "ok") return "109,247,181";
    if (tone === "warm") return "255,179,71";
    if (tone === "lost") return "255,74,139";
    return "244,247,242";
  }

  function chipToneForState(state) {
    if (state === "ok") return "ok";
    if (state === "relay") return "warm";
    if (state === "lost") return "lost";
    return "idle";
  }

  function colorForPrr(prr) {
    if (prr >= 0.76) return "var(--signal-300)";
    if (prr >= 0.26) return "var(--uplink-300)";
    return "var(--lost-300)";
  }

  function handleSend() {
    onSend({ target, kind, text, image: kind === 'image' ? imagePreview : null });
    text = "";
    imageFile = null;
    imagePreview = null;
  }
</script>

<div style="padding:20px 24px;overflow:auto;flex:1;display:grid;grid-template-columns:1fr 360px;gap:20px">
  <div>
    <div class="stamp" style="font-size:10px">03 / UPLINK</div>
    <div style="font-family:var(--font-display);font-size:24px;letter-spacing:-0.01em;color:var(--bone-100);margin-top:4px;margin-bottom:18px">
      Push a payload into the swarm
    </div>

    <!-- Target picker -->
    <div style="margin-bottom:18px">
      <div class="stamp" style="font-size:9px;margin-bottom:8px">TARGET NODE</div>
      <div style="display:flex;flex-wrap:wrap;gap:6px">
        {#each nodes.filter(n => n.id !== "HUB") as n}
          {@const active = target === n.id}
          {@const c = colorForState(n.state)}
          <button on:click={() => target = n.id}
            style="
              padding:6px 10px;
              border:1px solid {active ? c : 'var(--border)'};
              background: {active ? `rgba(${rgbOfTone(chipToneForState(n.state))},0.08)` : 'transparent'};
              color: {active ? c : 'var(--bone-200)'};
              font-family:var(--font-mono);font-size:11px;letter-spacing:0.08em;
              border-radius:2px;cursor:pointer;
              display:flex;align-items:center;gap:6px;
            ">
            <span style="width:6px;height:6px;border-radius:50%;background:{c};box-shadow: {active ? `0 0 6px ${c}` : 'none'}"></span>
            {n.label}
            <span style="opacity:0.6">· {n.prr.toFixed(2)}</span>
          </button>
        {/each}
      </div>
    </div>

    <!-- Kind picker -->
    <div style="margin-bottom:18px">
      <div class="stamp" style="font-size:9px;margin-bottom:8px">APPLICATION</div>
      <div style="display:flex;gap:6px">
        {#each [
          {id:"text", icon: MessageSquare, label:"Text"},
          {id:"image", icon: ImageIcon, label:"Image"},
          {id:"video", icon: Video, label:"Video"},
          {id:"file", icon: File, label:"File"}
        ] as k}
          <button on:click={() => kind = k.id}
            style="
              padding:10px 14px;
              border:1px solid {kind === k.id ? 'var(--signal-300)' : 'var(--border)'};
              background: {kind === k.id ? 'rgba(109,247,181,0.08)' : 'transparent'};
              color: {kind === k.id ? 'var(--signal-300)' : 'var(--bone-200)'};
              font-family:var(--font-mono);font-size:11px;letter-spacing:0.14em;text-transform:uppercase;
              border-radius:2px;cursor:pointer;display:flex;align-items:center;gap:8px;
            ">
            <svelte:component this={k.icon} size={14} />{k.label}
          </button>
        {/each}
      </div>
    </div>

    <!-- Payload editor -->
    <div style="margin-bottom:18px">
      <div class="stamp" style="font-size:9px;margin-bottom:8px">PAYLOAD</div>
      {#if kind === "text"}
        <textarea bind:value={text} placeholder="Type message..."
          style="
            width:100%;min-height:88px;background:var(--ink-100);border:1px solid var(--border);
            border-radius:2px;padding:12px 14px;color:var(--bone-100);
            font-family:var(--font-mono);font-size:13px;outline:none;resize:vertical;
          "></textarea>
      {:else if kind === "image"}
        <div style="position:relative">
          <input type="file" accept="image/*" on:change={handleFileSelect} 
            style="position:absolute;inset:0;opacity:0;cursor:pointer;z-index:2" />
          <div style="border:1px dashed {imagePreview ? 'var(--signal-300)' : 'var(--border-strong)'};border-radius:2px;padding:32px 14px;text-align:center;color:var(--bone-400);background:var(--ink-100)">
            {#if imagePreview}
              <img src={imagePreview} alt="preview" style="max-height:120px;max-width:100%;border-radius:2px;margin:0 auto;display:block;margin-bottom:12px" />
              <div class="mono" style="font-size:10px;color:var(--signal-300)">{imageFile?.name} · READY</div>
            {:else}
              <div style="font-family:var(--font-mono);font-size:12px;letter-spacing:0.14em;text-transform:uppercase">DROP IMAGE HERE</div>
              <div class="stamp" style="font-size:9px;margin-top:6px">OR CLICK TO BROWSE</div>
            {/if}
          </div>
        </div>
      {:else}
        <div style="border:1px dashed var(--border-strong);border-radius:2px;padding:32px 14px;text-align:center;color:var(--bone-400)">
          <div style="font-family:var(--font-mono);font-size:12px;letter-spacing:0.14em;text-transform:uppercase">DROP {kind.toUpperCase()} HERE</div>
          <div class="stamp" style="font-size:9px;margin-top:6px">OR CLICK TO BROWSE</div>
        </div>
      {/if}
    </div>

    <div style="display:flex;gap:10px;align-items:center">
      <button class="btn btn-primary" on:click={handleSend}>
        <Send size={14} /><span>SEND →</span>
      </button>
      <button class="btn btn-secondary">SCHEDULE</button>
      {#if lastResult}
        <div class="mono" style="font-size:11px;color: {lastResult.result === 'OK' ? 'var(--signal-300)' : lastResult.result === 'RELAY' ? 'var(--uplink-300)' : 'var(--lost-300)'}">
          ▸ {lastResult.message}
        </div>
      {/if}
    </div>
  </div>

  <!-- Side: route preview -->
  <div style="display:flex;flex-direction:column;gap:14px">
    <div class="panel bracketed" style="padding:16px;">
      <div class="h-eyebrow" style="margin-bottom:12px">ROUTE PREVIEW</div>
      <div class="mono" style="font-size:12px;color:var(--bone-300);margin-bottom:14px">
        HUB → {target}
      </div>
      <div style="display:flex;justify-content:space-between;margin-bottom:14px">
        <div>
          <div class="stamp" style="font-size:9px">EXPECTED PRR</div>
          <div class="ticker" style="font-size:22px;color:{colorForPrr(expectedPrr)};font-weight:500;margin-top:4px">
            {expectedPrr.toFixed(2)}
          </div>
        </div>
        <div>
          <div class="stamp" style="font-size:9px">HOPS</div>
          <div class="ticker" style="font-size:22px;color:var(--bone-100);font-weight:500;margin-top:4px">
            {targetNode ? (targetNode.hops || 1) : 0}
          </div>
        </div>
        <div>
          <div class="stamp" style="font-size:9px">ETA</div>
          <div class="ticker" style="font-size:22px;color:var(--bone-100);font-weight:500;margin-top:4px">
            {20 + (targetNode ? (targetNode.hops || 1) * 35 : 0)}ms
          </div>
        </div>
      </div>
      <div style="padding:10px;border:1px solid var(--border);border-radius:2px;background:var(--ink-050)">
        <div class="stamp" style="font-size:9px;margin-bottom:6px">FORECAST</div>
        <div class="mono" style="font-size:11px;color:{willLikelyLand ? 'var(--signal-300)' : 'var(--uplink-300)'}">
          {willLikelyLand ? "PACKET WILL LIKELY LAND." : "DEGRADED LINK — RETRY EXPECTED."}
        </div>
      </div>
    </div>

    <div class="panel bracketed" style="flex:1; display:flex; flex-direction:column; min-height:0; padding: 16px;">
      <div class="h-eyebrow" style="margin-bottom:12px">LIVE TRAFFIC</div>
      <div style="flex:1; min-height:0; display:flex; flex-direction:column">
        <PacketLog {entries} dense={true} />
      </div>
    </div>
  </div>
</div>
