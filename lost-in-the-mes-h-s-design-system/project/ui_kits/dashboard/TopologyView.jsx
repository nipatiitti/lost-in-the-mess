/* TopologyView.jsx — SVG mesh graph */

function TopologyView({ nodes, links, selected, setSelected, livePackets }) {
  const ref = React.useRef(null);
  const [box, setBox] = React.useState({ w: 800, h: 480 });

  React.useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(entries => {
      for (const e of entries) {
        setBox({ w: e.contentRect.width, h: e.contentRect.height });
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const byId = Object.fromEntries(nodes.map(n => [n.id, n]));

  const pos = (n) => ({ x: (n.x / 100) * box.w, y: (n.y / 100) * box.h });

  return (
    <div ref={ref} style={{position:"relative",flex:1,minHeight:0,overflow:"hidden",background:"var(--ink-050)"}}>
      {/* grid background */}
      <div className="grid-overlay" style={{position:"absolute",inset:0}}/>
      {/* scanlines */}
      <div className="scanlines" style={{position:"absolute",inset:0,pointerEvents:"none"}}/>

      {/* corner brackets */}
      <CornerBrackets/>

      {/* badge top-left */}
      <div style={{position:"absolute",top:16,left:24,zIndex:2}}>
        <div className="stamp" style={{fontSize:10,color:"var(--bone-300)"}}>02 / TOPOLOGY</div>
        <div style={{fontFamily:"var(--font-display)",fontSize:24,letterSpacing:"-0.01em",color:"var(--bone-100)",marginTop:4}}>14 nodes in range</div>
      </div>

      {/* legend bottom-right */}
      <div style={{position:"absolute",bottom:16,right:24,display:"flex",gap:8,zIndex:2}}>
        <Chip tone="ok">PRR ≥ 0.75</Chip>
        <Chip tone="warm">0.25 – 0.75</Chip>
        <Chip tone="lost">&lt; 0.25</Chip>
      </div>

      <svg width={box.w} height={box.h} style={{position:"absolute",inset:0}}>
        {/* halo behind hub */}
        <defs>
          <radialGradient id="halo" cx="50%" cy="50%" r="50%">
            <stop offset="0%" stopColor="#6DF7B5" stopOpacity="0.18"/>
            <stop offset="100%" stopColor="#6DF7B5" stopOpacity="0"/>
          </radialGradient>
        </defs>
        {nodes.filter(n => n.id === "HUB").map(n => {
          const p = pos(n);
          return <circle key="halo" cx={p.x} cy={p.y} r={Math.min(box.w, box.h) * 0.45} fill="url(#halo)"/>;
        })}

        {/* Links */}
        {links.map(([a, b, q], i) => {
          const A = byId[a], B = byId[b];
          if (!A || !B) return null;
          const pa = pos(A), pb = pos(B);
          const color = colorForPrr(q);
          const isLost = stateOfPrr(q) === "lost";
          const isRelay = stateOfPrr(q) === "relay";
          return (
            <line key={i} x1={pa.x} y1={pa.y} x2={pb.x} y2={pb.y}
              stroke={color}
              strokeOpacity={isLost ? 0.55 : isRelay ? 0.65 : 0.8}
              strokeWidth={isLost ? 1 : 1.2}
              strokeDasharray={isLost ? "2 4" : isRelay ? "4 3" : "0"}
              strokeLinecap="round"/>
          );
        })}

        {/* Live packets — small dots moving along link */}
        {livePackets.map(p => {
          const A = byId[p.from], B = byId[p.to];
          if (!A || !B) return null;
          const pa = pos(A), pb = pos(B);
          const t = p.t;
          const x = pa.x + (pb.x - pa.x) * t;
          const y = pa.y + (pb.y - pa.y) * t;
          return (
            <g key={p.id}>
              <circle cx={x} cy={y} r="6" fill="none" stroke="var(--signal-300)" strokeOpacity="0.4"/>
              <circle cx={x} cy={y} r="3" fill="var(--signal-300)"/>
            </g>
          );
        })}

        {/* Nodes */}
        {nodes.map(n => {
          const p = pos(n);
          const color = colorForState(n.state);
          const isHub = n.id === "HUB";
          const isSel = selected === n.id;
          const isLive = n.state === "ok";
          return (
            <g key={n.id}
               className={isLive ? "node-live" : ""}
               style={{cursor:"pointer"}}
               onClick={() => setSelected(n.id)}>
              {isHub && <circle cx={p.x} cy={p.y} r="22" fill="none" stroke="var(--signal-300)" strokeOpacity="0.3"/>}
              {isLive && <circle className="halo" cx={p.x} cy={p.y} r={isHub ? 18 : 12} fill={color} fillOpacity="0.18"/>}
              <circle cx={p.x} cy={p.y} r={isHub ? 12 : 6} fill="var(--ink-050)" stroke={color} strokeWidth={isSel ? 2 : 1.5}/>
              <circle cx={p.x} cy={p.y} r={isHub ? 6 : 3} fill={color}/>
              {isSel && (
                <circle cx={p.x} cy={p.y} r={isHub ? 22 : 14} fill="none" stroke={color}
                  strokeDasharray="3 3" strokeOpacity="0.7"/>
              )}
              {/* label */}
              <text x={p.x + (isHub ? 16 : 10)} y={p.y + 4}
                fontFamily="JetBrains Mono, monospace" fontSize="10"
                letterSpacing="0.14em" fill={color}>
                {n.label}
              </text>
              <text x={p.x + (isHub ? 16 : 10)} y={p.y + 16}
                fontFamily="JetBrains Mono, monospace" fontSize="9"
                fill="var(--bone-400)">
                {n.prr.toFixed(2)}
              </text>
            </g>
          );
        })}
      </svg>
    </div>
  );
}

function CornerBrackets() {
  const s = { position:"absolute", width:14, height:14, borderColor:"var(--bone-100)", borderStyle:"solid", opacity:0.5 };
  return (
    <>
      <div style={{...s, top:8,    left:8,    borderWidth:"1px 0 0 1px"}}/>
      <div style={{...s, top:8,    right:8,   borderWidth:"1px 1px 0 0"}}/>
      <div style={{...s, bottom:8, left:8,    borderWidth:"0 0 1px 1px"}}/>
      <div style={{...s, bottom:8, right:8,   borderWidth:"0 1px 1px 0"}}/>
    </>
  );
}

window.TopologyView = TopologyView;
