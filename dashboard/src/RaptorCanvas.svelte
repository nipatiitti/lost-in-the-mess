<script>
  import { onMount } from 'svelte';

  let canvas;
  let ctx;
  export let width = 100;
  export let height = 24;
  
  // Matrix state
  let progress = 0;
  let packets = [];
  
  onMount(() => {
    ctx = canvas.getContext('2d');
    
    // Connect to WebSocket
    const wsUrl = `ws://${window.location.hostname}:3000/api/raptor/ws`;
    const ws = new WebSocket(wsUrl);
    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        handleEvent(data);
      } catch (e) {
        console.error("Failed to parse message:", e);
      }
    };
    
    let frameId = requestAnimationFrame(draw);
    
    return () => {
      ws.close();
      cancelAnimationFrame(frameId);
    };
  });
  
  function handleEvent(data) {
    if (data.type === "PacketReceived") {
      packets.push({ ...data.data, t: Date.now(), alpha: 1.0, y: -5 });
      if (packets.length > 50) packets.shift();
    } else if (data.type === "DecoderStatus") {
      progress = data.data.progress;
    } else if (data.type === "DecodingSuccess" || data.type === "DecodingFailed") {
      progress = data.type === "DecodingSuccess" ? 1.0 : 0.0;
    }
  }

  function draw() {
    if (!ctx) return;
    
    // Clear background
    ctx.clearRect(0, 0, width, height);
    
    // Draw matrix representation at the bottom
    const mHeight = 3;
    ctx.fillStyle = '#333';
    ctx.fillRect(0, height - mHeight, width, mHeight);
    
    if (progress > 0) {
      ctx.fillStyle = `rgba(0, 255, 65, 0.8)`;
      ctx.fillRect(0, height - mHeight, width * progress, mHeight);
    }
    
    // Draw packets falling
    for (let i = 0; i < packets.length; i++) {
      let p = packets[i];
      p.y += 1.5;
      
      ctx.fillStyle = p.is_repair ? '#ffaa00' : '#00ff41';
      ctx.globalAlpha = p.alpha;
      
      // Packets drop randomly across the width
      const px = ((p.id + p.source_block) * 13) % width;
      
      if (p.y > height - mHeight && p.alpha > 0) {
         p.alpha -= 0.15; 
      }
      
      if (p.alpha > 0) {
          ctx.beginPath();
          ctx.arc(px, p.y, 1.5, 0, Math.PI * 2);
          ctx.fill();
      }
    }
    ctx.globalAlpha = 1.0;
    
    requestAnimationFrame(draw);
  }
</script>

<style>
  canvas {
    display: block;
    background: transparent;
    opacity: 0.8;
  }
</style>

<canvas bind:this={canvas} {width} {height} title="RaptorQ FEC Telemetry"></canvas>
