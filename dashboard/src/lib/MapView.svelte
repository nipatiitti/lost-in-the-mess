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
  let myX = 50 + (Math.random() - 0.5) * 20;
  let myY = 50 + (Math.random() - 0.5) * 20;

  // Tracked node positions: Map<nodeLabel, {x, y, timestamp}>
  let nodePositions = {};
  // Placed markers: [{x, y, text, node, timestamp}]
  let markers = [];

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

  // Reactively parse all messages for map data
  // Markers toggle: same =name sent again removes the previous one
  $: {
    const newPositions = {};
    const markerMap = new Map(); // key = text, toggle on/off
    
    // Process messages oldest-first (messages come in reversed)
    const chronological = [...messages].reverse();
    
    for (const msg of chronological) {
      const parsed = parseMapMessage(msg.payload);
      if (!parsed) continue;
      
      if (parsed.text) {
        // Toggle: if marker with same name exists, remove it; otherwise place it
        if (markerMap.has(parsed.text)) {
          markerMap.delete(parsed.text);
        } else {
          markerMap.set(parsed.text, {
            x: parsed.x,
            y: parsed.y,
            text: parsed.text,
            node: msg.node,
            time: msg.time
          });
        }
      }
      // Always update position (latest wins)
      newPositions[msg.node] = {
        x: parsed.x,
        y: parsed.y,
        timestamp: msg.time
      };
    }
    
    nodePositions = newPositions;
    markers = [...markerMap.values()];
  }

  // Convert logical coords to SVG pixels
  function toSvg(lx, ly) {
    const padTop = 108, padLeft = 96, padRight = 80, padBottom = 60;
    return {
      x: padLeft + (lx / MAP_W) * (svgWidth - padLeft - padRight),
      y: padTop + (ly / MAP_H) * (svgHeight - padTop - padBottom)
    };
  }

  // POI names for random marker drops
  const POI_NAMES = [
    'ALPHA', 'BRAVO', 'CHARLIE', 'DELTA', 'ECHO',
    'FOXTROT', 'GOLF', 'HOTEL', 'INDIA', 'JULIET',
    'KILO', 'LIMA', 'MIKE', 'NOVEMBER', 'OSCAR',
    'RALLY', 'OVERWATCH', 'EXFIL', 'LZ', 'CP'
  ];
  const POI_DROP_CHANCE = 0.2; // 20% chance per wander step

  // Semi-random wander: move to a nearby point
  function wander() {
    const angle = Math.random() * Math.PI * 2;
    const dist = Math.random() * WANDER_RADIUS;
    let nx = myX + Math.cos(angle) * dist;
    let ny = myY + Math.sin(angle) * dist;
    
    // Clamp to map bounds with some padding
    nx = Math.max(5, Math.min(MAP_W - 5, nx));
    ny = Math.max(5, Math.min(MAP_H - 5, ny));
    
    myX = Math.round(nx * 10) / 10;
    myY = Math.round(ny * 10) / 10;
    
    // Randomly drop or clear a POI marker
    if (Math.random() < POI_DROP_CHANCE) {
      const name = POI_NAMES[Math.floor(Math.random() * POI_NAMES.length)];
      broadcastMarker(name);
    } else {
      // Broadcast position only
      broadcastPosition();
    }
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
    const myLabel = `N-${localId.toString().padStart(2, '0')}`;
    
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
      if (label === myLabel) continue; // skip self
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

  // Grid line generation
  $: gridLinesX = Array.from({length: 11}, (_, i) => i * 10);
  $: gridLinesY = Array.from({length: 11}, (_, i) => i * 10);
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

  <!-- Position readout top-right -->
  <div style="position:absolute;top:16px;right:24px;z-index:2;text-align:right">
    <div class="stamp" style="font-size:9px;margin-bottom:4px">MY POSITION</div>
    <div class="ticker" style="font-size:18px;color:var(--signal-300);font-weight:500">
      [{myX.toFixed(1)}, {myY.toFixed(1)}]
    </div>
    <div class="stamp" style="font-size:9px;margin-top:4px;color:var(--bone-400)">
      WANDERING · {(WANDER_INTERVAL / 1000).toFixed(0)}s INTERVAL
    </div>
  </div>

  <!-- Legend bottom-right -->
  <div style="position:absolute;bottom:16px;right:24px;display:flex;gap:8px;z-index:2">
    <div class="chip chip-ok"><div class="chip-dot"></div>SELF</div>
    <div class="chip chip-info"><div class="chip-dot"></div>PEER</div>
    <div class="chip chip-warm"><div class="chip-dot"></div>MARKER</div>
  </div>

  <!-- Marker count bottom-left -->
  <div style="position:absolute;bottom:16px;left:24px;z-index:2">
    <div class="stamp" style="font-size:9px">
      {markers.length} MARKER{markers.length !== 1 ? 'S' : ''} · {Object.keys(nodePositions).length} TRACKED
    </div>
  </div>

  <!-- SVG map -->
  <svg width={svgWidth} height={svgHeight} style="position:absolute;inset:0">
    <!-- Coordinate grid lines -->
    {#each gridLinesX as gx}
      {@const p = toSvg(gx, 0)}
      {@const p2 = toSvg(gx, MAP_H)}
      <line x1={p.x} y1={p.y} x2={p2.x} y2={p2.y}
        stroke="var(--ink-400)" stroke-opacity="0.3" stroke-width="0.5"
        stroke-dasharray="2 6"></line>
      <text x={p.x} y={p.y - 6} font-family="JetBrains Mono, monospace"
        font-size="8" fill="var(--bone-500)" text-anchor="middle">{gx}</text>
    {/each}
    {#each gridLinesY as gy}
      {@const p = toSvg(0, gy)}
      {@const p2 = toSvg(MAP_W, gy)}
      <line x1={p.x} y1={p.y} x2={p2.x} y2={p2.y}
        stroke="var(--ink-400)" stroke-opacity="0.3" stroke-width="0.5"
        stroke-dasharray="2 6"></line>
      <text x={p.x - 8} y={p.y + 3} font-family="JetBrains Mono, monospace"
        font-size="8" fill="var(--bone-500)" text-anchor="end">{gy}</text>
    {/each}

    <!-- Map border -->
    <rect x={toSvg(0,0).x} y={toSvg(0,0).y}
      width={toSvg(MAP_W,MAP_H).x - toSvg(0,0).x}
      height={toSvg(MAP_W,MAP_H).y - toSvg(0,0).y}
      fill="none" stroke="var(--ink-500)" stroke-width="1"></rect>

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
</style>
