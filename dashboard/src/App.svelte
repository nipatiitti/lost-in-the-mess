<script>
  import { onMount } from 'svelte';
  import Sidebar from './lib/Sidebar.svelte';
  import StatusBar from './lib/StatusBar.svelte';
  import TabBar from './lib/TabBar.svelte';
  import TopologyView from './lib/TopologyView.svelte';
  import NodeGrid from './lib/NodeGrid.svelte';
  import PacketLog from './lib/PacketLog.svelte';
  import Composer from './lib/Composer.svelte';

  let data = {
    local_id: 0,
    neighbors: [],
    messages: []
  };

  let connected = false;
  let screen = "topology";
  const setScreen = (s) => screen = s;
  let selectedNode = null;
  let lastResult = null;
  let timeStr = "00:00:00";

  // Clock
  onMount(() => {
    const updateTime = () => {
      const now = new Date();
      timeStr = now.toISOString().split('T')[1].split('.')[0];
    };
    updateTime();
    const interval = setInterval(updateTime, 1000);
    return () => clearInterval(interval);
  });

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

  function getPrrState(prr) {
    if (prr >= 0.76) return "ok";
    if (prr >= 0.26) return "relay";
    return "lost";
  }

  $: nodes = (() => {
    const hubId = "HUB";
    const result = [{
      id: hubId,
      label: `N-${data.local_id.toString().padStart(2, '0')}`,
      prr: 1.0,
      state: "ok",
      dbm: 0,
      hops: 0,
      x: 50,
      y: 50
    }];

    const N = data.neighbors.length;
    data.neighbors.forEach((n, i) => {
      const angle = (i / N) * 2 * Math.PI;
      const radius = 30; // percentage
      result.push({
        id: `N-${n.id.toString().padStart(2, '0')}`,
        label: `N-${n.id.toString().padStart(2, '0')}`,
        prr: n.prr,
        state: getPrrState(n.prr),
        dbm: n.rssi_dbm,
        hops: 1, // still mock until mesh implements multi-hop Dijkstra export
        x: 50 + radius * Math.cos(angle),
        y: 50 + radius * Math.sin(angle)
      });
    });

    return result;
  })();

  $: formattedMessages = data.messages.map((m, i) => {
    const d = new Date(m.timestamp * 1000);
    return {
      id: m.id || i,
      time: d.toISOString().split('T')[1].split('.')[0],
      node: `N-${m.source.toString().padStart(2, '0')}`,
      nodeState: "ok",
      kind: "data",
      payload: m.text,
      result: "OK"
    };
  }).reverse();

  $: lastEvent = formattedMessages.length > 0 
    ? `${formattedMessages[0].node} · ${formattedMessages[0].kind} · ${formattedMessages[0].result}` 
    : "No recent events";

  async function handleSend({ target, kind, text }) {
    // Send to API
    try {
      const res = await fetch('/api/send', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ text })
      });
      if (res.ok) {
        lastResult = { result: "OK", message: `Packet committed → ${target}.` };
      } else {
        lastResult = { result: "LOST", message: `Packet lost on ${target}. Retry in 200ms.` };
      }
    } catch (e) {
      lastResult = { result: "LOST", message: `Network error. Failed to send.` };
    }
    setTimeout(() => lastResult = null, 4000);
  }
</script>

<div class="shell">
  <Sidebar {screen} {setScreen} {nodes} />
  
  <StatusBar time={timeStr} {lastEvent} onSend={() => screen = "uplink"} nodesCount={nodes.length} />
  
  <div class="shell-main">
    <TabBar {screen} setScreen={(s) => screen = s} nodesCount={nodes.length} />
    
    <div style="flex:1;min-height:0;display:flex;position:relative">
      <div style="flex:1;min-height:0;display:flex;flex-direction:column">
        {#if screen === "topology"}
          <TopologyView {nodes} selected={selectedNode} setSelected={(n) => selectedNode = n} />
        {:else if screen === "nodes"}
          <NodeGrid {nodes} selected={selectedNode} setSelected={(n) => selectedNode = n} />
        {:else if screen === "uplink"}
          <Composer {nodes} onSend={handleSend} {lastResult} />
        {:else if screen === "log"}
          <PacketLog entries={formattedMessages} />
        {/if}
      </div>
      
      <!-- Placeholder for NodeInspector if selectedNode && screen === "topology" -->
    </div>
  </div>
</div>
