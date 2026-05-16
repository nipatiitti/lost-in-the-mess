/* NodeGrid.jsx — grid of node cards */

function NodeGrid({ nodes, selected, setSelected }) {
  const [filter, setFilter] = React.useState("all");
  const counts = {
    all: nodes.length,
    ok: nodes.filter(n => n.state === "ok").length,
    relay: nodes.filter(n => n.state === "relay").length,
    lost: nodes.filter(n => n.state === "lost").length,
  };
  const filtered = filter === "all" ? nodes : nodes.filter(n => n.state === filter);

  return (
    <div style={{padding:"20px 24px",overflow:"auto",flex:1}}>
      <div style={{display:"flex",justifyContent:"space-between",alignItems:"center",marginBottom:18}}>
        <div>
          <div className="stamp" style={{fontSize:10}}>01 / NODES</div>
          <div style={{fontFamily:"var(--font-display)",fontSize:24,letterSpacing:"-0.01em",color:"var(--bone-100)",marginTop:4}}>
            {filtered.length} nodes — {filter === "all" ? "all states" : `state: ${filter}`}
          </div>
        </div>
        <div style={{display:"flex",gap:6}}>
          <FilterPill active={filter === "all"}   onClick={() => setFilter("all")}>All · {counts.all}</FilterPill>
          <FilterPill active={filter === "ok"}    tone="ok"   onClick={() => setFilter("ok")}>Nominal · {counts.ok}</FilterPill>
          <FilterPill active={filter === "relay"} tone="warm" onClick={() => setFilter("relay")}>Relay · {counts.relay}</FilterPill>
          <FilterPill active={filter === "lost"}  tone="lost" onClick={() => setFilter("lost")}>Lost · {counts.lost}</FilterPill>
        </div>
      </div>

      <div style={{display:"grid",gridTemplateColumns:"repeat(auto-fill,minmax(220px,1fr))",gap:12}}>
        {filtered.map(n => (
          <NodeCard key={n.id} node={n} selected={selected === n.id} onClick={() => setSelected(n.id)}/>
        ))}
      </div>
    </div>
  );
}

function FilterPill({ active, tone = "idle", onClick, children }) {
  const baseColor = active ? colorOfTone(tone) : "var(--bone-300)";
  const borderColor = active ? colorOfTone(tone) : "var(--border)";
  const bg = active && tone !== "idle" ? `rgba(${rgbOfTone(tone)},0.08)` : "transparent";
  return (
    <button onClick={onClick}
      style={{padding:"5px 10px",background:bg,color:baseColor,border:`1px solid ${borderColor}`,
              fontFamily:"var(--font-mono)",fontSize:10,letterSpacing:"0.14em",textTransform:"uppercase",
              borderRadius:999,cursor:"pointer"}}>
      {children}
    </button>
  );
}
function colorOfTone(t) {
  return t === "ok" ? "var(--signal-300)"
       : t === "warm" ? "var(--uplink-300)"
       : t === "lost" ? "var(--lost-300)"
       : "var(--bone-100)";
}
function rgbOfTone(t) {
  return t === "ok" ? "109,247,181"
       : t === "warm" ? "255,179,71"
       : t === "lost" ? "255,74,139"
       : "244,247,242";
}

function NodeCard({ node, selected, onClick }) {
  const color = colorForState(node.state);
  const tone = chipToneForState(node.state);
  const label = labelForState(node.state);
  const isLost = node.state === "lost";

  return (
    <div onClick={onClick}
      style={{
        position:"relative",
        background:"var(--bg-panel)",
        border: selected ? `1px solid ${color}` : "1px solid var(--border)",
        borderRadius:4,
        padding:14,
        cursor:"pointer",
        boxShadow: isLost ? "0 0 0 1px rgba(255,74,139,0.20), 0 0 12px rgba(255,74,139,0.18)" : "none",
        transition:"border-color 120ms var(--ease-snap)",
      }}>
      <div style={{display:"flex",justifyContent:"space-between",alignItems:"flex-start",marginBottom:10}}>
        <div>
          <div style={{fontFamily:"var(--font-mono)",fontSize:18,color:"var(--bone-100)"}}>{node.label}</div>
          <div className="stamp" style={{fontSize:9,marginTop:2}}>QUAD · 2.4GHz · {node.role.toUpperCase()}</div>
        </div>
        <Chip tone={tone}>{label}</Chip>
      </div>
      <div style={{display:"flex",justifyContent:"space-between",alignItems:"baseline"}}>
        <div className="ticker" style={{fontSize:28,color,fontWeight:500}}>{node.prr.toFixed(2)}</div>
        <div className="stamp" style={{fontSize:9}}>PRR</div>
      </div>
      <div style={{marginTop:8,height:3,background:"var(--ink-200)",borderRadius:1,overflow:"hidden"}}>
        <div style={{width:`${node.prr*100}%`,height:"100%",background:color}}/>
      </div>
      <div className="mono" style={{fontSize:10,color:"var(--bone-300)",marginTop:10,display:"flex",justifyContent:"space-between"}}>
        <span>{node.dbm}dBm</span>
        <span>{node.hops === 0 ? "HUB" : `${node.hops} HOP${node.hops>1?"S":""}`}</span>
      </div>
    </div>
  );
}

window.NodeGrid = NodeGrid;
window.NodeCard = NodeCard;
