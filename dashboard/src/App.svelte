<script>
  import { onMount } from 'svelte';
  import Metrics from './lib/Metrics.svelte';
  import NodeList from './lib/NodeList.svelte';
  import MessageLog from './lib/MessageLog.svelte';
  import SendBox from './lib/SendBox.svelte';

  let data = {
    local_id: 0,
    neighbors: [],
    messages: []
  };

  let connected = false;

  async function fetchData() {
    try {
      const res = await fetch('/api/data');
      if (res.ok) {
        data = await res.json();
        connected = true;
      } else {
        connected = false;
      }
    } catch (e) {
      connected = false;
    }
  }

  onMount(() => {
    fetchData();
    const interval = setInterval(fetchData, 500);
    return () => clearInterval(interval);
  });
</script>

<main class="min-h-screen p-4 md:p-8 space-y-6">
  <!-- Header -->
  <header class="flex justify-between items-center border-b border-[#00ff41]/30 pb-4">
    <div>
      <h1 class="text-2xl font-bold tracking-widest neon-glow">LITM TACTICAL MESH</h1>
      <p class="text-xs opacity-60">STATION ID: {data.local_id} | NODE_ID_{data.local_id.toString().padStart(3, '0')}</p>
    </div>
    <div class="flex items-center space-x-2">
      <div class={`w-3 h-3 rounded-full ${connected ? 'bg-[#00ff41] animate-pulse' : 'bg-red-600'}`}></div>
      <span class="text-xs uppercase font-bold">{connected ? 'System Online' : 'System Offline'}</span>
    </div>
  </header>

  <!-- Grid Layout -->
  <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
    <!-- Left Column: Metrics & Send -->
    <div class="space-y-6">
      <Metrics {data} />
      <SendBox />
    </div>

    <!-- Middle Column: Node Graph / List -->
    <div class="lg:col-span-2 space-y-6">
      <div class="glass-card p-4 min-h-[400px]">
        <h2 class="text-sm font-bold uppercase mb-4 opacity-70">Active Neighbors</h2>
        <NodeList neighbors={data.neighbors} />
      </div>
      
      <div class="glass-card p-4">
        <h2 class="text-sm font-bold uppercase mb-4 opacity-70">Signal Log</h2>
        <MessageLog messages={data.messages} />
      </div>
    </div>
  </div>
</main>

<style>
  :global(*) {
    scrollbar-width: thin;
    scrollbar-color: #00ff41 #0a0a0a;
  }
</style>
