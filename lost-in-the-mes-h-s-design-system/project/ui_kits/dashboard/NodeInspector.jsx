/* NodeInspector.jsx — right-side drawer with per-node detail */

function NodeInspector({ node, onClose, links, nodes }) {
  if (!node) return null;
  const color = colorForState(node.state);
  const tone = chipToneForState(node.state);
  const stateLabel = labelForState(node.state);
  const series = React.useMemo(() => sparkFor(node.id, node.prr), [node.id, node.prr]);

  // peer links
  const peers = links
    .filter(([a, b]) => a === node.id || b === node.id)
    .map(([a, b, q]) => {
      const other = a === node.id ? b : a;
      return { id: other, q, state: stateOfPrr(q) };
    });

  return (
    <div style={{
      position:"absolute",top:0,right:0,bottom:0,width:380,
      background:"var(--ink-100)",borderLeft:"1px solid var(--border)",
      display:"flex",flexDirection:"column",zIndex:20,
      boxShadow:"-24px 0 64px rgba(0,0,0,0.55)",
    }}>
      {/* header */}
      <div style={{padding:"16px 18px",borderBottom:"1px solid var(--border)",display:"flex",alignItems:"flex-start",justifyContent:"space-between"}}>
        <div>
          <div className="stamp" style={{fontSize:9}}>NODE INSPECT</div>
          <div style={{display:"flex",alignItems:"center",gap:10,marginTop:6}}>
            <div style={{fontFamily:"var(--font-display)",fontSize:28,letterSpacing:"-0.02em",color:"var(--bone-100)"}}>{node.label}</div>
            <Chip tone={tone} glow={node.state === "ok"}>{stateLabel}</Chip>
          </div>
          <div className="mono" style={{fontSize:11,color:"var(--bone-300)",marginTop:6}}>
            {node.role.toUpperCase()} · QUAD · 2.4GHz
          </div>
        </div>
        <IconButton title="Close" onClick={onClose}>{I.close}</IconButton>
      </div>

      {/* metrics */}
      <div style={{padding:"18px",display:"grid",gridTemplateColumns:"1fr 1fr",gap:14,borderBottom:"1px solid var(--border)"}}>
        <Metric label="PRR" value={node.prr.toFixed(2)} color={color}/>
        <Metric label="RSSI" value={`${node.dbm}dBm`}/>
        <Metric label="HOPS" value={node.hops === 0 ? "HUB" : node.hops}/>
        <Metric label="UPTIME" value="04h22"/>
      </div>

      {/* sparkline */}
      <div style={{padding:"18px",borderBottom:"1px solid var(--border)"}}>
        <div style={{display:"flex",justifyContent:"space-between",alignItems:"baseline",marginBottom:10}}>
          <div className="stamp" style={{fontSize:9}}>PRR · LAST 24 TICKS</div>
          <div className="mono" style={{fontSize:11,color:"var(--bone-300)"}}>min {Math.min(...series).toFixed(2)} · max {Math.max(...series).toFixed(2)}</div>
        </div>
        <Sparkline data={series} color={color} w={344} h={48}/>
      </div>

      {/* peers */}
      <div style={{padding:"18px",flex:1,overflow:"auto"}}>
        <div className="stamp" style={{fontSize:9,marginBottom:10}}>PEER LINKS · {peers.length}</div>
        <div style={{display:"flex",flexDirection:"column",gap:6}}>
          {peers.map(p => (
            <div key={p.id} style={{display:"grid",gridTemplateColumns:"60px 1fr 50px",alignItems:"center",gap:10,padding:"8px 10px",border:"1px solid var(--border)",borderRadius:2,background:"var(--ink-050)"}}>
              <div style={{fontFamily:"var(--font-mono)",fontSize:12,color:"var(--bone-100)"}}>{p.id}</div>
              <div style={{position:"relative",height:4,background:"var(--ink-200)",borderRadius:1,overflow:"hidden"}}>
                <div style={{width:`${p.q*100}%`,height:"100%",background:colorForPrr(p.q)}}/>
              </div>
              <div className="mono" style={{fontSize:11,color:colorForPrr(p.q),textAlign:"right"}}>{p.q.toFixed(2)}</div>
            </div>
          ))}
        </div>
      </div>

      {/* actions */}
      <div style={{padding:"14px 18px",borderTop:"1px solid var(--border)",display:"flex",gap:8}}>
        <Button variant="primary" style={{flex:1,justifyContent:"center"}}>{I.send}<span>UPLINK</span></Button>
        <Button variant="secondary">REROUTE</Button>
      </div>
    </div>
  );
}

function Metric({ label, value, color }) {
  return (
    <div>
      <div className="stamp" style={{fontSize:9,marginBottom:6}}>{label}</div>
      <div className="ticker" style={{fontSize:22,color:color || "var(--bone-100)",fontWeight:500}}>{value}</div>
    </div>
  );
}

window.NodeInspector = NodeInspector;
