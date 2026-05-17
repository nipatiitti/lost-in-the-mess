<script>
  import { onMount, onDestroy } from 'svelte';

  export let entries = [];
  export let dense = false;

  let lightboxImage = null;
  let lightboxMeta = null;

  function colorForState(state) {
    if (state === "ok") return "var(--signal-300)";
    if (state === "relay") return "var(--uplink-300)";
    if (state === "lost") return "var(--lost-300)";
    return "var(--bone-100)";
  }

  function openLightbox(entry) {
    lightboxImage = entry.image;
    lightboxMeta = { node: entry.node, time: entry.time, kind: entry.kind };
  }

  function closeLightbox() {
    lightboxImage = null;
    lightboxMeta = null;
  }

  function handleKeydown(e) {
    if (e.key === 'Escape' && lightboxImage) {
      closeLightbox();
    }
  }

  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
  });
  onDestroy(() => {
    window.removeEventListener('keydown', handleKeydown);
  });
</script>

{#if lightboxImage}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <div class="lightbox-overlay" on:click={closeLightbox}>
    <div class="lightbox-content" on:click|stopPropagation>
      <div class="lightbox-header">
        <div class="lightbox-meta">
          <span class="lightbox-chip" style="color:{colorForState('ok')}">{lightboxMeta.node}</span>
          <span class="lightbox-time">{lightboxMeta.time}</span>
          <span class="lightbox-kind">{lightboxMeta.kind}</span>
        </div>
        <button class="lightbox-close" on:click={closeLightbox}>✕</button>
      </div>
      <div class="lightbox-img-wrap">
        <img src={lightboxImage} alt="Full image" class="lightbox-img" />
      </div>
      <div class="lightbox-hint">CLICK OUTSIDE OR PRESS ESC TO CLOSE</div>
    </div>
  </div>
{/if}

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
    <div style="display:grid;grid-template-columns:{dense ? '50px 1fr 60px' : '90px 60px 70px 1fr 70px'};padding:8px 14px;background:var(--ink-200);color:var(--bone-400);letter-spacing:0.14em;font-family:var(--font-mono);font-size:9px;text-transform:uppercase;border-bottom:1px solid var(--border);position:sticky;top:0;z-index:1">
      {#if !dense}<div>UTC</div>{/if}
      <div>NODE</div>
      {#if !dense}<div>KIND</div>{/if}
      <div>PAYLOAD</div>
      <div style="text-align:right">STATE</div>
    </div>
    {#each entries as e (e.id)}
      <div style="display:grid;grid-template-columns:{dense ? '50px 1fr 60px' : '90px 60px 70px 1fr 70px'};padding:7px 14px;font-family:var(--font-mono);font-size:11px;border-bottom:1px solid var(--ink-200);align-items:center">
        {#if !dense}<div style="color:var(--bone-500)">{e.time}</div>{/if}
        <div style="color:{colorForState(e.nodeState || 'ok')}">{e.node}</div>
        {#if !dense}<div style="color:var(--bone-100)">{e.kind}</div>{/if}
        <div style="color:var(--bone-300);overflow:hidden;display:flex;align-items:center;gap:8px" title={e.payload}>
          {#if e.image}
            <!-- svelte-ignore a11y-click-events-have-key-events -->
            <img
              src={e.image}
              alt="attachment"
              class="pktlog-thumb"
              on:click={() => openLightbox(e)}
            />
          {/if}
          <span style="white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{e.payload}</span>
        </div>
        <div style="text-align:right;color:{colorForState(e.result === 'OK' ? 'ok' : e.result === 'RELAY' ? 'relay' : 'lost')}">{e.result}</div>
      </div>
    {/each}
  </div>
</div>

<style>
  /* Thumbnail in log row */
  .pktlog-thumb {
    height: 18px;
    width: auto;
    border: 1px solid var(--border);
    border-radius: 1px;
    flex-shrink: 0;
    cursor: pointer;
    transition: transform 120ms ease, box-shadow 120ms ease, border-color 120ms ease;
  }
  .pktlog-thumb:hover {
    transform: scale(1.25);
    border-color: var(--signal-300);
    box-shadow: 0 0 8px rgba(109,247,181,0.35);
    z-index: 5;
    position: relative;
  }

  /* Lightbox overlay */
  .lightbox-overlay {
    position: fixed;
    inset: 0;
    z-index: 9999;
    background: rgba(4, 6, 10, 0.88);
    backdrop-filter: blur(12px);
    display: flex;
    align-items: center;
    justify-content: center;
    animation: lb-fade-in 200ms ease;
  }

  .lightbox-content {
    max-width: 94vw;
    max-height: 94vh;
    display: flex;
    flex-direction: column;
    gap: 12px;
    animation: lb-scale-in 200ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }

  .lightbox-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .lightbox-meta {
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  .lightbox-chip {
    padding: 3px 10px;
    border: 1px solid rgba(109,247,181,0.4);
    background: rgba(109,247,181,0.08);
    border-radius: 999px;
    font-size: 10px;
    letter-spacing: 0.14em;
  }

  .lightbox-time {
    color: var(--bone-400);
  }

  .lightbox-kind {
    color: var(--bone-300);
  }

  .lightbox-close {
    width: 32px;
    height: 32px;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 2px;
    color: var(--bone-200);
    font-size: 14px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 120ms ease, border-color 120ms ease;
  }
  .lightbox-close:hover {
    background: var(--ink-300);
    border-color: var(--bone-400);
  }

  .lightbox-img-wrap {
    border: 1px solid var(--border);
    border-radius: 2px;
    overflow: hidden;
    background: var(--ink-100);
    box-shadow: 0 24px 64px rgba(0,0,0,0.7), 0 1px 0 rgba(255,255,255,0.05) inset;
  }

  .lightbox-img {
    display: block;
    max-width: 92vw;
    max-height: 82vh;
    object-fit: contain;
    margin: 0 auto;
  }

  .lightbox-hint {
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    color: var(--bone-500);
    text-align: center;
  }

  @keyframes lb-fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  @keyframes lb-scale-in {
    from { opacity: 0; transform: scale(0.95); }
    to { opacity: 1; transform: scale(1); }
  }
</style>
