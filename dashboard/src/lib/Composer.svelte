<script>
  import { onDestroy } from "svelte";
  import {
    MessageSquare,
    Image as ImageIcon,
    Video,
    Send,
  } from "@lucide/svelte";
  import PacketLog from "./PacketLog.svelte";
  export let nodes = [];
  export let entries = [];
  export let lastResult = null;
  export let onSend = () => {};

  let target = nodes.length > 1 ? nodes.find((n) => n.id !== "HUB")?.id : "";
  let kind = "text";
  let text = "";
  let imageFile = null;
  let imagePreview = null;

  // Video streaming state
  let videoStream = null;
  let videoEl = null;
  let canvasEl = null;
  let streaming = false;
  let streamInterval = null;
  let frameCount = 0;

  const CAPTURE_WIDTH = 320;
  const CAPTURE_HEIGHT = 320;
  const STREAM_FPS = 3;

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

  $: targetNode = nodes.find((n) => n.id === target);
  $: expectedPrr = targetNode ? targetNode.prr : 1;
  $: willLikelyLand = expectedPrr >= 0.6;

  // When kind changes away from video, stop camera
  $: if (kind !== "video") {
    stopStreaming();
    stopCamera();
  }

  // When kind changes to video, start camera
  $: if (kind === "video") {
    startCamera();
  }

  // Reactively bind the stream to the video element whenever both become available
  $: if (videoEl && videoStream) {
    videoEl.srcObject = videoStream;
  }

  async function startCamera() {
    if (videoStream) return;
    try {
      videoStream = await navigator.mediaDevices.getUserMedia({
        video: { width: { ideal: 640 }, height: { ideal: 480 } },
      });
    } catch (e) {
      console.error("Camera access denied:", e);
    }
  }

  function stopCamera() {
    if (videoStream) {
      videoStream.getTracks().forEach((t) => t.stop());
      videoStream = null;
    }
    if (videoEl) {
      videoEl.srcObject = null;
    }
  }

  function startStreaming() {
    if (streaming) return;
    streaming = true;
    frameCount = 0;
    streamInterval = setInterval(captureAndSend, 1000 / STREAM_FPS);
  }

  function stopStreaming() {
    streaming = false;
    if (streamInterval) {
      clearInterval(streamInterval);
      streamInterval = null;
    }
  }

  async function captureAndSend() {
    if (!videoEl || !canvasEl || !videoStream) return;

    const ctx = canvasEl.getContext("2d");
    canvasEl.width = CAPTURE_WIDTH;
    canvasEl.height = CAPTURE_HEIGHT;
    ctx.drawImage(videoEl, 0, 0, CAPTURE_WIDTH, CAPTURE_HEIGHT);

    // Get JPEG as data URI, then extract the base64 part
    const dataUri = canvasEl.toDataURL("image/jpeg", 0.6);
    const b64 = dataUri.split(",")[1];

    try {
      const res = await fetch("/api/video/frame", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          width: CAPTURE_WIDTH,
          height: CAPTURE_HEIGHT,
          data: b64,
        }),
      });
      if (res.ok) {
        frameCount++;
      }
    } catch (e) {
      console.error("Failed to send video frame:", e);
    }
  }

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
    if (kind === "video") {
      if (streaming) {
        stopStreaming();
        lastResult = {
          result: "OK",
          message: `Stream stopped. ${frameCount} frames sent.`,
        };
        setTimeout(() => (lastResult = null), 4000);
      } else {
        startStreaming();
        lastResult = { result: "OK", message: "Streaming started..." };
      }
      return;
    }
    onSend({
      target,
      kind,
      text,
      image: kind === "image" ? imagePreview : null,
    });
    text = "";
    imageFile = null;
    imagePreview = null;
  }

  function handleKeydown(e) {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      handleSend();
    }
  }

  onDestroy(() => {
    stopStreaming();
    stopCamera();
  });
</script>

<canvas bind:this={canvasEl} style="display:none"></canvas>

<div
  style="padding:20px 24px;overflow:auto;flex:1;display:grid;grid-template-columns:1fr 360px;gap:20px"
>
  <div>
    <div class="stamp" style="font-size:10px">03 / UPLINK</div>
    <div
      style="font-family:var(--font-display);font-size:24px;letter-spacing:-0.01em;color:var(--bone-100);margin-top:4px;margin-bottom:18px"
    >
      Push a payload into the swarm
    </div>

    <!-- Target picker -->
    <div style="margin-bottom:18px">
      <div class="stamp" style="font-size:9px;margin-bottom:8px">
        TARGET NODE
      </div>
      <div style="display:flex;flex-wrap:wrap;gap:6px">
        {#each nodes.filter((n) => n.id !== "HUB") as n}
          {@const active = target === n.id}
          {@const c = colorForState(n.state)}
          <button
            on:click={() => (target = n.id)}
            style="
              padding:6px 10px;
              border:1px solid {active ? c : 'var(--border)'};
              background: {active
              ? `rgba(${rgbOfTone(chipToneForState(n.state))},0.08)`
              : 'transparent'};
              color: {active ? c : 'var(--bone-200)'};
              font-family:var(--font-mono);font-size:11px;letter-spacing:0.08em;
              border-radius:2px;cursor:pointer;
              display:flex;align-items:center;gap:6px;
            "
          >
            <span
              style="width:6px;height:6px;border-radius:50%;background:{c};box-shadow: {active
                ? `0 0 6px ${c}`
                : 'none'}"
            ></span>
            {n.label}
            <span style="opacity:0.6">· {n.prr.toFixed(2)}</span>
          </button>
        {/each}
      </div>
    </div>

    <!-- Kind picker -->
    <div style="margin-bottom:18px">
      <div class="stamp" style="font-size:9px;margin-bottom:8px">
        APPLICATION
      </div>
      <div style="display:flex;gap:6px">
        {#each [{ id: "text", icon: MessageSquare, label: "Text" }, { id: "image", icon: ImageIcon, label: "Image" }, { id: "video", icon: Video, label: "Video" }] as k}
          <button
            on:click={() => (kind = k.id)}
            style="
              padding:10px 14px;
              border:1px solid {kind === k.id
              ? 'var(--signal-300)'
              : 'var(--border)'};
              background: {kind === k.id
              ? 'rgba(109,247,181,0.08)'
              : 'transparent'};
              color: {kind === k.id ? 'var(--signal-300)' : 'var(--bone-200)'};
              font-family:var(--font-mono);font-size:11px;letter-spacing:0.14em;text-transform:uppercase;
              border-radius:2px;cursor:pointer;display:flex;align-items:center;gap:8px;
            "
          >
            <svelte:component this={k.icon} size={14} />{k.label}
          </button>
        {/each}
      </div>
    </div>

    <!-- Payload editor -->
    <div style="margin-bottom:18px">
      <div class="stamp" style="font-size:9px;margin-bottom:8px">PAYLOAD</div>
      {#if kind === "text"}
        <textarea
          bind:value={text}
          on:keydown={handleKeydown}
          placeholder="Type message..."
          style="
            width:100%;min-height:88px;background:var(--ink-100);border:1px solid var(--border);
            border-radius:2px;padding:12px 14px;color:var(--bone-100);
            font-family:var(--font-mono);font-size:13px;outline:none;resize:vertical;
          "
        ></textarea>
        <div
          class="stamp"
          style="font-size:9px;margin-top:6px;text-align:right;opacity:0.6"
        >
          PRESS ⌘/⌃ + ↵ TO TRANSMIT
        </div>
      {:else if kind === "image"}
        <div style="position:relative">
          <input
            type="file"
            accept="image/*"
            on:change={handleFileSelect}
            style="position:absolute;inset:0;opacity:0;cursor:pointer;z-index:2"
          />
          <div
            style="border:1px dashed {imagePreview
              ? 'var(--signal-300)'
              : 'var(--border-strong)'};border-radius:2px;padding:32px 14px;text-align:center;color:var(--bone-400);background:var(--ink-100)"
          >
            {#if imagePreview}
              <img
                src={imagePreview}
                alt="preview"
                style="max-height:320px;max-width:100%;border-radius:2px;margin:0 auto;display:block;margin-bottom:12px"
              />
              <div class="mono" style="font-size:10px;color:var(--signal-300)">
                {imageFile?.name} · READY
              </div>
            {:else}
              <div
                style="font-family:var(--font-mono);font-size:12px;letter-spacing:0.14em;text-transform:uppercase"
              >
                DROP IMAGE HERE
              </div>
              <div class="stamp" style="font-size:9px;margin-top:6px">
                OR CLICK TO BROWSE
              </div>
            {/if}
          </div>
        </div>
      {:else if kind === "video"}
        <div
          style="border:1px solid {streaming
            ? 'var(--signal-300)'
            : 'var(--border)'};border-radius:2px;overflow:hidden;background:#000;position:relative"
        >
          <!-- svelte-ignore a11y-media-has-caption -->
          <video
            bind:this={videoEl}
            autoplay
            playsinline
            muted
            style="
              width:100%;max-height:400px;display:block;object-fit:cover;
              transform:scaleX(-1);
            "
          ></video>
          {#if streaming}
            <div
              style="
              position:absolute;top:8px;right:8px;
              display:flex;align-items:center;gap:6px;
              padding:4px 10px;border-radius:2px;
              background:rgba(0,0,0,0.6);backdrop-filter:blur(4px);
            "
            >
              <span
                style="width:8px;height:8px;border-radius:50%;background:var(--lost-300);animation:pulse-dot 1s ease infinite"
              ></span>
              <span
                style="font-family:var(--font-mono);font-size:10px;letter-spacing:0.12em;color:var(--lost-300)"
              >
                LIVE · {frameCount} FRAMES
              </span>
            </div>
          {:else}
            <div
              style="
              position:absolute;bottom:8px;left:0;right:0;text-align:center;
            "
            >
              <span
                style="
                padding:4px 12px;border-radius:2px;
                background:rgba(0,0,0,0.6);backdrop-filter:blur(4px);
                font-family:var(--font-mono);font-size:10px;letter-spacing:0.12em;color:var(--bone-200);
              "
                >PREVIEW · {CAPTURE_WIDTH}×{CAPTURE_HEIGHT} @ {STREAM_FPS}FPS</span
              >
            </div>
          {/if}
        </div>
      {/if}
    </div>

    <div style="display:flex;gap:10px;align-items:center">
      <button class="btn btn-primary" on:click={handleSend}>
        {#if kind === "video"}
          {#if streaming}
            <span
              style="width:12px;height:12px;border-radius:2px;background:var(--lost-300);display:inline-block"
            ></span>
            <span>STOP ■</span>
          {:else}
            <Send size={14} /><span>STREAM →</span>
          {/if}
        {:else}
          <Send size={14} /><span>SEND →</span>
        {/if}
      </button>
      {#if lastResult}
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

  <!-- Side: route preview -->
  <div style="display:flex;flex-direction:column;gap:14px">
    <div class="panel bracketed" style="padding:16px;">
      <div class="h-eyebrow" style="margin-bottom:12px">ROUTE PREVIEW</div>
      <div
        class="mono"
        style="font-size:12px;color:var(--bone-300);margin-bottom:14px"
      >
        HUB → {target}
      </div>
      <div
        style="display:flex;justify-content:space-between;margin-bottom:14px"
      >
        <div>
          <div class="stamp" style="font-size:9px">EXPECTED PRR</div>
          <div
            class="ticker"
            style="font-size:22px;color:{colorForPrr(
              expectedPrr,
            )};font-weight:500;margin-top:4px"
          >
            {expectedPrr.toFixed(2)}
          </div>
        </div>
        <div>
          <div class="stamp" style="font-size:9px">HOPS</div>
          <div
            class="ticker"
            style="font-size:22px;color:var(--bone-100);font-weight:500;margin-top:4px"
          >
            {targetNode ? targetNode.hops || 1 : 0}
          </div>
        </div>
        <div>
          <div class="stamp" style="font-size:9px">ETA</div>
          <div
            class="ticker"
            style="font-size:22px;color:var(--bone-100);font-weight:500;margin-top:4px"
          >
            {20 + (targetNode ? (targetNode.hops || 1) * 35 : 0)}ms
          </div>
        </div>
      </div>
      <div
        style="padding:10px;border:1px solid var(--border);border-radius:2px;background:var(--ink-050)"
      >
        <div class="stamp" style="font-size:9px;margin-bottom:6px">
          FORECAST
        </div>
        <div
          class="mono"
          style="font-size:11px;color:{willLikelyLand
            ? 'var(--signal-300)'
            : 'var(--uplink-300)'}"
        >
          {willLikelyLand
            ? "PACKET WILL LIKELY LAND."
            : "DEGRADED LINK — RETRY EXPECTED."}
        </div>
      </div>
    </div>

    <div
      class="panel bracketed"
      style="flex:1; display:flex; flex-direction:column; min-height:0; padding: 16px;"
    >
      <div class="h-eyebrow" style="margin-bottom:12px">LIVE TRAFFIC</div>
      <div style="flex:1; min-height:0; display:flex; flex-direction:column">
        <PacketLog {entries} dense={true} />
      </div>
    </div>
  </div>
</div>

<style>
  @keyframes pulse-dot {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.3;
    }
  }
</style>
