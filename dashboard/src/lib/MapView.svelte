<script>
  import { onMount, onDestroy } from 'svelte';
  export let messages = []; // formattedMessages from App
  export let localId = 0;
  export let onSend = () => {};
  export let connected = false;

  // Map config
  const MAP_W = 100; // logical grid width
  const MAP_H = 100; // logical grid height
  const WANDER_INTERVAL = 3000; // ms between position broadcasts
  const WANDER_RADIUS = 8; // max distance per step
  const MARKER_TIMEOUT = 0; // markers persist forever (0 = no timeout)

  let svgContainer;
  let svgWidth = 800;
  let svgHeight = 600;
  let wanderTimer = null;

  // Our position (start roughly center with some randomness)
  let myX = Math.round((50 + (Math.random() - 0.5) * 20) * 10) / 10;
  let myY = Math.round((50 + (Math.random() - 0.5) * 20) * 10) / 10;

  // Tracked node positions: Map<nodeLabel, {x, y, timestamp}>
  let nodePositions = {};
  // All markers (own local + other nodes from messages)
  let markers = [];
  // Own markers tracked locally for instant UI (no roundtrip needed)
  let ownMarkerMap = new Map(); // name -> {x, y, text, node, time}

  // Parse messages for [x,y] or [x,y]=text format
  const MAP_REGEX = /^\[(\d+(?:\.\d+)?)\s*,\s*(\d+(?:\.\d+)?)\](?:=(.+))?$/;

  function parseMapMessage(text) {
    if (!text) return null;
    const m = text.match(MAP_REGEX);
    if (!m) return null;
    return {
      x: parseFloat(m[1]),
      y: parseFloat(m[2]),
      text: m[3] ? m[3].trim() : null
    };
  }

  // Reactively parse messages — only OTHER nodes' markers (explicit place/remove),
  // own markers come from local ownMarkerMap (instant, no roundtrip)
  $: myLabel = `N-${localId.toString().padStart(2, '0')}`;

  $: {
    const newPositions = {};
    // key = "node|name", last-writer-wins (no toggle)
    const otherMarkerMap = new Map();
    
    // messages are newest-first; iterate that way so first-seen = newest wins
    for (const msg of messages) {
      const parsed = parseMapMessage(msg.payload);
      if (!parsed) continue;
      
      // Only process markers from OTHER nodes
      if (parsed.text && msg.node !== myLabel) {
        const isRemoval = parsed.text.startsWith('-');
        const markerName = isRemoval ? parsed.text.slice(1) : parsed.text;
        const key = `${msg.node}|${markerName}`;
        
        // Last-writer-wins: skip if we already saw a newer message for this key
        if (!otherMarkerMap.has(key)) {
          if (isRemoval) {
            // Mark as removed (null) so earlier place messages don't resurrect it
            otherMarkerMap.set(key, null);
          } else {
            otherMarkerMap.set(key, {
              x: parsed.x,
              y: parsed.y,
              text: markerName,
              node: msg.node,
              time: msg.time
            });
          }
        }
      }
      // Update position (newest-first, so first seen = latest)
      if (!newPositions[msg.node]) {
        newPositions[msg.node] = {
          x: parsed.x,
          y: parsed.y,
          timestamp: msg.time
        };
      }
    }
    
    nodePositions = newPositions;
    // Merge: other nodes' active markers + our own local markers
    const otherActive = [...otherMarkerMap.values()].filter(v => v !== null);
    markers = [...otherActive, ...ownMarkerMap.values()];
  }

  // Convert logical coords to SVG pixels
  function toSvg(lx, ly) {
    const padTop = 108, padLeft = 96, padRight = 80, padBottom = 60;
    return {
      x: padLeft + (lx / MAP_W) * (svgWidth - padLeft - padRight),
      y: padTop + (ly / MAP_H) * (svgHeight - padTop - padBottom)
    };
  }

  // POI names for marker drops
  const POI_NAMES = [
    'ALPHA', 'BRAVO', 'CHARLIE', 'DELTA', 'ECHO',
    'FOXTROT', 'GOLF', 'HOTEL', 'INDIA', 'JULIET',
    'KILO', 'LIMA', 'MIKE', 'NOVEMBER', 'OSCAR',
    'RALLY', 'OVERWATCH', 'EXFIL', 'LZ', 'CP'
  ];
  let nextPoiIndex = Math.floor(Math.random() * POI_NAMES.length);

  // Semi-random wander: move to a nearby point
  function wander() {
    const angle = Math.random() * Math.PI * 2;
    const dist = Math.random() * WANDER_RADIUS;
    let nx = myX + Math.cos(angle) * dist;
    let ny = myY + Math.sin(angle) * dist;
    
    nx = Math.max(5, Math.min(MAP_W - 5, nx));
    ny = Math.max(5, Math.min(MAP_H - 5, ny));
    
    myX = Math.round(nx * 10) / 10;
    myY = Math.round(ny * 10) / 10;
    
    broadcastPosition();
  }

  // Drop a marker at current position (instant local + broadcast)
  function dropMarker() {
    const name = POI_NAMES[nextPoiIndex % POI_NAMES.length];
    nextPoiIndex++;
    ownMarkerMap.set(name, {
      x: myX, y: myY, text: name, node: myLabel, time: 'NOW'
    });
    ownMarkerMap = ownMarkerMap; // trigger Svelte reactivity
    broadcastMarker(name);
  }

  // Remove own marker (instant local + broadcast explicit removal)
  function removeMarker(name) {
    ownMarkerMap.delete(name);
    ownMarkerMap = ownMarkerMap; // trigger Svelte reactivity
    broadcastMarker('-' + name); // '-' prefix = remove
  }

  function broadcastPosition() {
    if (!connected) return;
    onSend({
      target: 'ALL',
      kind: 'text',
      text: `[${myX},${myY}]`,
      image: null
    });
  }

  function broadcastMarker(name) {
    if (!connected) return;
    onSend({
      target: 'ALL',
      kind: 'text',
      text: `[${myX},${myY}]=${name}`,
      image: null
    });
  }

  onMount(() => {
    // ResizeObserver
    const ro = new ResizeObserver(entries => {
      for (const e of entries) {
        svgWidth = e.contentRect.width;
        svgHeight = e.contentRect.height;
      }
    });
    if (svgContainer) ro.observe(svgContainer);

    // Start wandering
    // Send initial position immediately
    setTimeout(() => broadcastPosition(), 500);
    wanderTimer = setInterval(wander, WANDER_INTERVAL);

    return () => {
      ro.disconnect();
      if (wanderTimer) clearInterval(wanderTimer);
    };
  });

  onDestroy(() => {
    if (wanderTimer) {
      clearInterval(wanderTimer);
      wanderTimer = null;
    }
  });

  // Build node list for rendering
  $: allNodes = (() => {
    const result = [];
    
    // Add self
    result.push({
      label: myLabel,
      x: myX,
      y: myY,
      isSelf: true,
      time: null
    });
    
    // Add other nodes from parsed positions
    for (const [label, pos] of Object.entries(nodePositions)) {
      if (label === myLabel) continue;
      result.push({
        label,
        x: pos.x,
        y: pos.y,
        isSelf: false,
        time: pos.timestamp
      });
    }
    
    return result;
  })();

  // Own markers from local state (instant)
  $: ownMarkers = [...ownMarkerMap.values()];


</script>

<div bind:this={svgContainer} style="position:relative;flex:1;min-height:0;overflow:hidden;background:var(--ink-050)">
  <!-- grid background -->
  <div class="grid-overlay" style="position:absolute;inset:0"></div>
  <div class="scanlines" style="position:absolute;inset:0;pointer-events:none"></div>

  <!-- corner brackets -->
  <div style="position:absolute;width:14px;height:14px;border-color:var(--bone-100);border-style:solid;opacity:0.5;top:8px;left:8px;border-width:1px 0 0 1px"></div>
  <div style="position:absolute;width:14px;height:14px;border-color:var(--bone-100);border-style:solid;opacity:0.5;top:8px;right:8px;border-width:1px 1px 0 0"></div>
  <div style="position:absolute;width:14px;height:14px;border-color:var(--bone-100);border-style:solid;opacity:0.5;bottom:8px;left:8px;border-width:0 0 1px 1px"></div>
  <div style="position:absolute;width:14px;height:14px;border-color:var(--bone-100);border-style:solid;opacity:0.5;bottom:8px;right:8px;border-width:0 1px 1px 0"></div>

  <!-- badge top-left -->
  <div style="position:absolute;top:16px;left:24px;z-index:2">
    <div class="stamp" style="font-size:10px;color:var(--bone-300)">05 / MAP</div>
    <div style="font-family:var(--font-display);font-size:24px;letter-spacing:-0.01em;color:var(--bone-100);margin-top:4px">
      Tactical map · {allNodes.length} node{allNodes.length !== 1 ? 's' : ''}
    </div>
  </div>

  <!-- Position readout + controls top-right -->
  <div style="position:absolute;top:16px;right:24px;z-index:2;text-align:right">
    <div class="stamp" style="font-size:9px;margin-bottom:4px">MY POSITION</div>
    <div class="ticker" style="font-size:18px;color:var(--signal-300);font-weight:500">
      [{myX.toFixed(1)}, {myY.toFixed(1)}]
    </div>
    <div class="stamp" style="font-size:9px;margin-top:4px;color:var(--bone-400)">
      WANDERING · {(WANDER_INTERVAL / 1000).toFixed(0)}s INTERVAL
    </div>
    <button class="map-drop-btn" on:click={dropMarker} disabled={!connected}
      title="Place a named marker at your current position">
      <span style="font-size:12px">◆</span> DROP MARKER
    </button>
  </div>

  <!-- Own markers panel bottom-right -->
  <div class="map-own-markers" style="position:absolute;bottom:16px;right:24px;z-index:2">
    <div style="display:flex;gap:8px;align-items:center;margin-bottom:{ownMarkers.length ? 6 : 0}px">
      <div class="chip chip-ok"><div class="chip-dot"></div>SELF</div>
      <div class="chip chip-info"><div class="chip-dot"></div>PEER</div>
      <div class="chip chip-warm"><div class="chip-dot"></div>MARKER</div>
    </div>
    {#if ownMarkers.length}
      <div class="own-marker-list">
        {#each ownMarkers as m (m.text)}
          <button class="own-marker-item" on:click={() => removeMarker(m.text)}
            title="Click to remove marker {m.text}">
            <span class="own-marker-diamond">◆</span>
            <span class="own-marker-name">{m.text}</span>
            <span class="own-marker-coord">[{m.x.toFixed(1)},{m.y.toFixed(1)}]</span>
            <span class="own-marker-x">✕</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Marker count bottom-left -->
  <div style="position:absolute;bottom:16px;left:24px;z-index:2">
    <div class="stamp" style="font-size:9px">
      {markers.length} MARKER{markers.length !== 1 ? 'S' : ''} · {ownMarkers.length} OWN · {Object.keys(nodePositions).length} TRACKED
    </div>
  </div>

  <!-- SVG map -->
  <svg width={svgWidth} height={svgHeight} style="position:absolute;inset:0">


    <!-- Markers -->
    {#each markers as marker}
      {@const mp = toSvg(marker.x, marker.y)}
      <!-- Marker pin -->
      <g class="map-marker">
        <!-- Diamond shape -->
        <polygon points="{mp.x},{mp.y-10} {mp.x+6},{mp.y} {mp.x},{mp.y+10} {mp.x-6},{mp.y}"
          fill="rgba(255,179,71,0.15)" stroke="var(--uplink-300)" stroke-width="1"></polygon>
        <circle cx={mp.x} cy={mp.y} r="2.5" fill="var(--uplink-300)"></circle>
        <!-- Label background -->
        <rect x={mp.x + 10} y={mp.y - 9} width={marker.text.length * 7 + 12} height="18"
          rx="2" fill="rgba(7,11,18,0.85)" stroke="var(--uplink-300)" stroke-opacity="0.5" stroke-width="0.5"></rect>
        <!-- Label text -->
        <text x={mp.x + 16} y={mp.y + 3}
          font-family="JetBrains Mono, monospace" font-size="10"
          letter-spacing="0.06em" fill="var(--uplink-300)">{marker.text}</text>
        <!-- Source node -->
        <text x={mp.x + 16} y={mp.y + 14}
          font-family="JetBrains Mono, monospace" font-size="7"
          fill="var(--bone-500)">{marker.node} · {marker.time}</text>
      </g>
    {/each}

    <!-- Trail lines between consecutive positions of same node -->
    <!-- (future enhancement) -->

    <!-- Nodes -->
    {#each allNodes as node (node.label)}
      {@const np = toSvg(node.x, node.y)}
      {@const color = node.isSelf ? 'var(--signal-300)' : 'var(--drift-300)'}
      <g class={node.isSelf ? 'map-self-node' : ''}>
        <!-- Range halo for self -->
        {#if node.isSelf}
          <circle cx={np.x} cy={np.y} r="30"
            fill="none" stroke="var(--signal-300)" stroke-opacity="0.12"
            stroke-dasharray="3 4"></circle>
          <circle cx={np.x} cy={np.y} r="16"
            fill={color} fill-opacity="0.08"></circle>
        {/if}
        
        <!-- Outer ring -->
        <circle cx={np.x} cy={np.y} r={node.isSelf ? 8 : 6}
          fill="var(--ink-050)" stroke={color} stroke-width="1.5"></circle>
        <!-- Inner dot -->
        <circle cx={np.x} cy={np.y} r={node.isSelf ? 4 : 3}
          fill={color}></circle>
        
        <!-- Pulsing halo for self -->
        {#if node.isSelf}
          <circle class="halo" cx={np.x} cy={np.y} r="12"
            fill={color} fill-opacity="0.18"></circle>
        {/if}
        
        <!-- Label -->
        <text x={np.x + (node.isSelf ? 14 : 10)} y={np.y - 4}
          font-family="JetBrains Mono, monospace"
          font-size={node.isSelf ? "11" : "10"}
          font-weight={node.isSelf ? "600" : "400"}
          letter-spacing="0.14em" fill={color}>{node.label}</text>
        <!-- Coordinates -->
        <text x={np.x + (node.isSelf ? 14 : 10)} y={np.y + 8}
          font-family="JetBrains Mono, monospace" font-size="8"
          fill="var(--bone-400)">[{node.x.toFixed(1)},{node.y.toFixed(1)}]</text>
      </g>
    {/each}
  </svg>
</div>

<style>
  .map-self-node circle.halo {
    animation: nodePulse 2s var(--ease-in-out) infinite;
  }
  
  @keyframes nodePulse {
    0%, 100% { opacity: 0.55; }
    50% { opacity: 1; }
  }
  
  .map-marker {
    transition: opacity 0.3s ease;
  }
  .map-marker:hover {
    opacity: 0.8;
  }

  /* Drop marker button */
  .map-drop-btn {
    margin-top: 10px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    border: 1px solid var(--uplink-300);
    border-radius: 4px;
    background: rgba(255, 179, 71, 0.1);
    color: var(--uplink-300);
    font-family: 'JetBrains Mono', monospace;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.12em;
    cursor: pointer;
    transition: background 0.2s, box-shadow 0.2s;
  }
  .map-drop-btn:hover:not(:disabled) {
    background: rgba(255, 179, 71, 0.22);
    box-shadow: 0 0 12px rgba(255, 179, 71, 0.15);
  }
  .map-drop-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  /* Own markers list */
  .own-marker-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-height: 140px;
    overflow-y: auto;
  }
  .own-marker-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    border: 1px solid rgba(255, 179, 71, 0.25);
    border-radius: 3px;
    background: rgba(7, 11, 18, 0.75);
    color: var(--bone-200);
    font-family: 'JetBrains Mono', monospace;
    font-size: 9px;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s;
    text-align: left;
  }
  .own-marker-item:hover {
    background: rgba(255, 70, 70, 0.12);
    border-color: rgba(255, 70, 70, 0.5);
  }
  .own-marker-item:hover .own-marker-x {
    opacity: 1;
    color: #ff5050;
  }
  .own-marker-diamond {
    color: var(--uplink-300);
    font-size: 8px;
  }
  .own-marker-name {
    color: var(--uplink-300);
    font-weight: 600;
    letter-spacing: 0.08em;
    min-width: 60px;
  }
  .own-marker-coord {
    color: var(--bone-500);
    font-size: 8px;
  }
  .own-marker-x {
    margin-left: auto;
    opacity: 0.3;
    font-size: 10px;
    transition: opacity 0.15s, color 0.15s;
  }
</style>
