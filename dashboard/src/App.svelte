<script>
  import { onMount } from 'svelte';
  import Sidebar from './lib/Sidebar.svelte';
  import StatusBar from './lib/StatusBar.svelte';
  import TopologyView from './lib/TopologyView.svelte';
  import NodeGrid from './lib/NodeGrid.svelte';
  import PacketLog from './lib/PacketLog.svelte';
  import Composer from './lib/Composer.svelte';
  import Streams from './lib/Streams.svelte';


  let data = {
    local_id: 0,
    neighbors: [],
    messages: [],
    topology: {},
    radio: null
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

  let fetching = false;
  async function fetchData() {
    if (fetching) return;
    fetching = true;
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
    } finally {
      fetching = false;
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
    if (!connected) return [];
    
    const hubId = "HUB";
    const result = [];
    const nodeMap = new Map();

    const addNode = (id, obj) => {
      if (!nodeMap.has(id)) {
        nodeMap.set(id, obj);
        result.push(obj);
      }
      return nodeMap.get(id);
    };

    addNode(data.local_id, {
      id: hubId,
      label: `N-${(data.local_id || 0).toString().padStart(2, '0')}`,
      prr: 1.0,
      state: "ok",
      dbm: 0,
      hops: 0,
      x: 50,
      y: 50
    });

    if (data.neighbors) {
      data.neighbors.forEach(n => {
        addNode(n.id, {
          id: `N-${n.id.toString().padStart(2, '0')}`,
          label: `N-${n.id.toString().padStart(2, '0')}`,
          prr: n.prr,
          state: getPrrState(n.prr),
          dbm: n.rssi_dbm,
          hops: 1
        });
      });
    }

    if (data.topology) {
      Object.entries(data.topology).forEach(([sourceStr, edges]) => {
        const sourceId = parseInt(sourceStr, 10);
        addNode(sourceId, {
          id: `N-${sourceId.toString().padStart(2, '0')}`,
          label: `N-${sourceId.toString().padStart(2, '0')}`,
          prr: 0,
          state: "lost",
          dbm: -100,
          hops: 2
        });
        
        edges.forEach(([targetId, prr]) => {
          addNode(targetId, {
            id: `N-${targetId.toString().padStart(2, '0')}`,
            label: `N-${targetId.toString().padStart(2, '0')}`,
            prr: 0,
            state: "lost",
            dbm: -100,
            hops: 2
          });
        });
      });

      // Simple BFS for hops
      let bfsQueue = [data.local_id];
      let visited = new Set([data.local_id]);
      
      while(bfsQueue.length > 0) {
         let curr = bfsQueue.shift();
         let currNode = nodeMap.get(curr);
         let currHops = currNode ? currNode.hops : 0;
         
         let neighborsOfCurr = [];
         if (curr === data.local_id) {
            neighborsOfCurr = (data.neighbors || []).map(n => n.id);
         } else if (data.topology[curr]) {
            neighborsOfCurr = data.topology[curr].map(link => link[0]);
         }
         
         for (let nId of neighborsOfCurr) {
            if (!visited.has(nId) && nodeMap.has(nId)) {
               visited.add(nId);
               nodeMap.get(nId).hops = currHops + 1;
               bfsQueue.push(nId);
            }
         }
      }
    }

    // Layout
    const hopsArr = [];
    result.forEach(n => {
       if (n.id === "HUB") return;
       let h = n.hops;
       if (!hopsArr[h]) hopsArr[h] = [];
       hopsArr[h].push(n);
    });

    hopsArr.forEach((nodesAtHop, h) => {
       if (!nodesAtHop) return;
       const N = Math.max(nodesAtHop.length, 3);
       const radius = Math.min(20 * h, 45); // cap radius
       nodesAtHop.forEach((n, i) => {
          const angle = (i / N) * 2 * Math.PI + (h * 0.5);
          n.x = 50 + radius * Math.cos(angle);
          n.y = 50 + radius * Math.sin(angle);
       });
    });

    return result;
  })();

  $: meshLinks = (() => {
    let l = [];
    if (!connected) return l;
    if (data.neighbors) {
       data.neighbors.forEach(n => {
          l.push({ source: "HUB", target: `N-${n.id.toString().padStart(2, '0')}`, prr: n.prr });
       });
    }
    if (data.topology) {
       Object.entries(data.topology).forEach(([sourceStr, edges]) => {
          const sourceId = parseInt(sourceStr, 10);
          const sId = sourceId === data.local_id ? "HUB" : `N-${sourceId.toString().padStart(2, '0')}`;
          
          edges.forEach(([targetId, prr]) => {
             const tId = targetId === data.local_id ? "HUB" : `N-${targetId.toString().padStart(2, '0')}`;
             l.push({ source: sId, target: tId, prr: prr });
          });
       });
    }
    return l;
  })();

  $: formattedMessages = data.messages.map((m, i) => {
    const d = new Date(m.timestamp * 1000);
    const nodeLabel = `N-${m.source.toString().padStart(2, '0')}`;
    const sourceNode = nodes.find(n => n.label === nodeLabel);
    
    return {
      id: m.id || i,
      time: d.toISOString().split('T')[1].split('.')[0],
      node: nodeLabel,
      nodeState: sourceNode ? sourceNode.state : "ok",
      kind: m.image ? "image" : "text",
      payload: m.image ? "IMAGE ATTACHMENT" : (m.text !== null && m.text !== "" ? m.text : "EMPTY"),
      image: m.image,
      result: "RECV"
    };
  }).reverse();

  $: lastEvent = formattedMessages.length > 0 
    ? `${formattedMessages[0].node} · ${formattedMessages[0].kind} · ${formattedMessages[0].result}` 
    : "No recent events";

  async function handleSend({ target, kind, text, image }) {
    // Send to API
    try {
      const res = await fetch('/api/send', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ text, image })
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
  <Sidebar {screen} {setScreen} {nodes} activeStreamsCount={data.active_streams ? data.active_streams.length : 0} {connected} />
  
  <StatusBar 
    time={timeStr} 
    {lastEvent} 
    onSend={() => screen = "uplink"} 
    nodesCount={nodes.length} 
    {connected} 
    radio={data.radio}
  />
  
  <div class="shell-main">
    <div style="flex:1;min-height:0;display:flex;position:relative;flex-direction:column">
        {#if screen === "topology"}
          <TopologyView {nodes} links={meshLinks} selected={selectedNode} setSelected={(n) => selectedNode = n} />
        {:else if screen === "nodes"}
          <NodeGrid {nodes} selected={selectedNode} setSelected={(n) => selectedNode = n} />
        {:else if screen === "uplink"}
          <Composer {nodes} onSend={handleSend} {lastResult} entries={formattedMessages} />
        {:else if screen === "streams"}
          <Streams activeStreams={data.active_streams || []} {nodes} />
        {:else if screen === "log"}
          <PacketLog entries={formattedMessages} />
        {/if}
    </div>
  </div>
</div>
