<script>
  export let activeStreams = [];
  export let nodes = [];
  
  function getLabel(id) {
    const n = nodes.find(n => n.id === `N-${id.toString().padStart(2, '0')}`);
    return n ? n.label : `N-${id.toString().padStart(2, '0')}`;
  }
</script>

<div style="padding: 24px; overflow-y: auto; flex: 1;">
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
            style="width: 100%; display: block; aspect-ratio: 4/3; object-fit: contain; background: #111;" 
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

<style>
  @keyframes pulse-dot {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }
</style>
