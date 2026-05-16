<script>
  import { onMount } from 'svelte';

  let canvas;
  let ctx;
  let width = 800;
  let height = 600;
  
  // Matrix state
  let matrix = { rows: 0, cols: 0, density: 0 };
  let progress = 0;
  let overhead = 0;
  let packets = [];
  
  let connected = false;
  
  onMount(() => {
    ctx = canvas.getContext('2d');
    
    // Connect to WebSocket
    const wsUrl = `ws://${window.location.hostname}:3000/api/raptor/ws`;
    const ws = new WebSocket(wsUrl);
    ws.onopen = () => { connected = true; };
    ws.onclose = () => { connected = false; };
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
      packets.push({ ...data.data, t: Date.now(), alpha: 1.0, y: -20 });
      if (packets.length > 500) packets.shift();
    } else if (data.type === "DecoderStatus") {
      progress = data.data.progress;
      overhead = data.data.overhead_symbols;
    } else if (data.type === "MatrixState") {
      matrix = data.data;
    } else if (data.type === "DecodingSuccess" || data.type === "DecodingFailed") {
      progress = data.type === "DecodingSuccess" ? 1.0 : 0.0;
      matrix.density = progress;
    }
  }

  function draw() {
    if (!ctx) return;
    
    // Clear background
    ctx.fillStyle = '#0a0a0a';
    ctx.fillRect(0, 0, width, height);
    
    const now = Date.now();
    
    // Draw matrix representation
    ctx.strokeStyle = '#333';
    ctx.lineWidth = 2;
    const mSize = 300;
    const mx = width / 2 - mSize / 2;
    const my = height / 2 - mSize / 2 + 50;
    
    ctx.strokeRect(mx, my, mSize, mSize);
    
    if (matrix.rows > 0) {
      ctx.fillStyle = `rgba(0, 255, 65, ${matrix.density * 0.4})`;
      ctx.fillRect(mx, my + mSize * (1 - matrix.density), mSize, mSize * matrix.density);
    }
    
    // Draw packets falling
    for (let i = 0; i < packets.length; i++) {
      let p = packets[i];
      let age = now - p.t;
      
      p.y += 3;
      
      ctx.fillStyle = p.is_repair ? '#ffaa00' : '#00ff41';
      ctx.globalAlpha = p.alpha;
      
      // Packets drop towards matrix bounds
      const px = mx + ((p.id + p.source_block) % 20) * (mSize / 20) + (p.t % 5);
      
      if (p.y > my + mSize * (1 - Math.random() * matrix.density) && p.alpha > 0) {
         p.alpha -= 0.1; 
      }
      
      if (p.alpha > 0) {
          ctx.beginPath();
          ctx.arc(px, p.y, 4, 0, Math.PI * 2);
          ctx.fill();
      }
    }
    ctx.globalAlpha = 1.0;
    
    // Text stats
    ctx.fillStyle = '#00ff41';
    ctx.font = '16px monospace';
    ctx.fillText(`WebSocket: ${connected ? 'CONNECTED' : 'DISCONNECTED'}`, 20, 30);
    ctx.fillText(`Progress: ${(progress * 100).toFixed(1)}%`, 20, 60);
    ctx.fillText(`Overhead: ${overhead} symbols`, 20, 90);
    ctx.fillText(`Matrix Size: ${matrix.rows}x${matrix.cols}`, 20, 120);
    
    requestAnimationFrame(draw);
  }
</script>

<style>
  .container {
    width: 100vw;
    height: 100vh;
    display: flex;
    justify-content: center;
    align-items: center;
    background-color: #0a0a0a;
    color: #00ff41;
  }
  canvas {
    border: 1px solid #333;
    background-color: #000;
    border-radius: 8px;
    box-shadow: 0 0 20px rgba(0, 255, 65, 0.1);
  }
</style>

<div class="container">
  <canvas bind:this={canvas} {width} {height}></canvas>
</div>
