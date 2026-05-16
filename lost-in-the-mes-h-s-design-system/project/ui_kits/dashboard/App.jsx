/* App.jsx — top-level shell, routing, fake telemetry simulator */

const { useState, useEffect, useRef, useMemo } = React;

const INITIAL_LOG = [
  { id:1, time:"22:14:07", node:"N-04", nodeState:"ok",    kind:"image", payload:"survey_NE_quad.jpg · 1.2MB · 14 chunks", result:"OK" },
  { id:2, time:"22:14:05", node:"N-07", nodeState:"ok",    kind:"text",  payload:"\"holding pattern, awaiting nav\"", result:"OK" },
  { id:3, time:"22:14:03", node:"N-09", nodeState:"relay", kind:"video", payload:"stream_a.h264 · ch.2 · degraded", result:"RELAY" },
  { id:4, time:"22:14:02", node:"N-11", nodeState:"lost",  kind:"image", payload:"survey_NW.jpg · ch.7 dropped · retry 200ms", result:"LOST" },
  { id:5, time:"22:14:01", node:"HUB",  nodeState:"ok",    kind:"route", payload:"→ N-04 → N-07 committed", result:"OK" },
  { id:6, time:"22:13:55", node:"N-13", nodeState:"ok",    kind:"text",  payload:"\"telemetry sync\"", result:"OK" },
  { id:7, time:"22:13:50", node:"N-11", nodeState:"lost",  kind:"text",  payload:"\"beacon lost\"", result:"LOST" },
  { id:8, time:"22:13:44", node:"N-04", nodeState:"ok",    kind:"image", payload:"survey_S.jpg · 0.8MB · 9 chunks", result:"OK" },
];

function App() {
  const [screen, setScreen] = useState("topology");
  const [nodes, setNodes] = useState(INITIAL_NODES);
  const [links] = useState(INITIAL_LINKS);
  const [selected, setSelected] = useState(null);
  const [log, setLog] = useState(INITIAL_LOG);
  const [livePackets, setLivePackets] = useState([]);
  const [lastResult, setLastResult] = useState(null);
  const [time, setTime] = useState("22:14:09");
  const seq = useRef(100);

  /* live clock + small PRR jitter */
  useEffect(() => {
    const id = setInterval(() => {
      setTime(t => {
        const [h,m,s] = t.split(":").map(Number);
        let sx = s + 1, mx = m, hx = h;
        if (sx === 60) { sx = 0; mx += 1; }
        if (mx === 60) { mx = 0; hx += 1; }
        return `${String(hx).padStart(2,"0")}:${String(mx).padStart(2,"0")}:${String(sx).padStart(2,"0")}`;
      });
      setNodes(ns => ns.map(n => {
        if (n.id === "HUB") return n;
        const jitter = (Math.random() - 0.5) * 0.04;
        const prr = Math.max(0, Math.min(1, n.prr + jitter));
        return { ...n, prr, state: stateOfPrr(prr) };
      }));
    }, 1500);
    return () => clearInterval(id);
  }, []);

  /* spawn a packet animation every 2s on a random OK link */
  useEffect(() => {
    const id = setInterval(() => {
      const candidates = links.filter(([a,b,q]) => q >= 0.6);
      if (!candidates.length) return;
      const pick = candidates[Math.floor(Math.random() * candidates.length)];
      const pid = ++seq.current;
      setLivePackets(p => [...p, { id: pid, from: pick[0], to: pick[1], t: 0 }]);
      const start = performance.now();
      const dur = 1200;
      const step = (now) => {
        const t = Math.min(1, (now - start) / dur);
        setLivePackets(p => p.map(x => x.id === pid ? { ...x, t } : x));
        if (t < 1) requestAnimationFrame(step);
        else setLivePackets(p => p.filter(x => x.id !== pid));
      };
      requestAnimationFrame(step);
    }, 1800);
    return () => clearInterval(id);
  }, [links]);

  const handleSend = ({ target, kind, text }) => {
    const node = nodes.find(n => n.id === target);
    if (!node) return;
    const willLand = node.prr >= 0.6;
    const isDegraded = node.prr >= 0.25 && node.prr < 0.6;
    const result = willLand ? "OK" : isDegraded ? "RELAY" : "LOST";
    const payload = kind === "text"
      ? `"${text.slice(0, 40)}${text.length > 40 ? "…" : ""}"`
      : `${kind}_${Math.floor(Math.random()*9000+1000)}.${kind === "image" ? "jpg" : kind === "video" ? "h264" : "bin"} · ${(Math.random()*2+0.4).toFixed(1)}MB`;
    const entry = {
      id: ++seq.current,
      time,
      node: target,
      nodeState: node.state,
      kind,
      payload,
      result,
    };
    setLog(l => [entry, ...l]);
    setLastResult({
      result,
      message: result === "OK" ? `Packet committed → ${target}.`
             : result === "RELAY" ? `Relaying via N-04. PRR degraded.`
             : `Packet lost on ${target}. Retry in 200ms.`,
    });
    setTimeout(() => setLastResult(null), 4000);
  };

  const selectedNode = nodes.find(n => n.id === selected);

  const screens = {
    topology: (
      <>
        <TopologyView nodes={nodes} links={links} selected={selected} setSelected={setSelected} livePackets={livePackets}/>
      </>
    ),
    nodes:    <NodeGrid nodes={nodes} selected={selected} setSelected={setSelected}/>,
    uplink:   <Composer nodes={nodes} onSend={handleSend} lastResult={lastResult}/>,
    log:      <PacketLog entries={log}/>,
    apps:     <UplinkApps onLaunch={() => setScreen("uplink")}/>,
  };

  return (
    <div className="shell">
      <Sidebar screen={screen} setScreen={setScreen} nodes={nodes}/>
      <StatusBar time={time} lastEvent={`${log[0].node} · ${log[0].kind} · ${log[0].result}`} onSend={() => setScreen("uplink")}/>
      <div className="shell-main">
        <TabBar screen={screen} setScreen={setScreen}/>
        <div style={{flex:1,minHeight:0,display:"flex",position:"relative"}}>
          <div style={{flex:1,minHeight:0,display:"flex",flexDirection:"column"}}>
            {screens[screen]}
          </div>
          {selectedNode && screen === "topology" && (
            <NodeInspector node={selectedNode} links={links} nodes={nodes} onClose={() => setSelected(null)}/>
          )}
        </div>
      </div>
    </div>
  );
}

function TabBar({ screen, setScreen }) {
  const tabs = [
    { id: "topology", label: "Topology" },
    { id: "nodes",    label: "Nodes · 14" },
    { id: "uplink",   label: "Uplink" },
    { id: "log",      label: "Log" },
    { id: "apps",     label: "Apps · 4" },
  ];
  return (
    <div className="tabbar">
      {tabs.map(t => (
        <button key={t.id} onClick={() => setScreen(t.id)} className={`tab ${screen === t.id ? "active" : ""}`}>
          {t.label}
        </button>
      ))}
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App/>);
