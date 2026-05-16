/* Sidebar.jsx — Left rail nav + brand + mesh health summary */

function Sidebar({ screen, setScreen, nodes }) {
  const ok    = nodes.filter(n => n.state === "ok").length;
  const relay = nodes.filter(n => n.state === "relay").length;
  const lost  = nodes.filter(n => n.state === "lost").length;
  const avgPrr = (nodes.reduce((a,n) => a + n.prr, 0) / nodes.length).toFixed(2);

  const items = [
    { id: "topology", label: "Topology", icon: I.layers,   count: nodes.length },
    { id: "nodes",    label: "Nodes",    icon: I.grid,     count: nodes.length },
    { id: "uplink",   label: "Uplink",   icon: I.send,     count: null },
    { id: "log",      label: "Log",      icon: I.activity, count: "23s" },
    { id: "apps",     label: "Apps",     icon: I.layers,   count: 3 },
  ];

  return (
    <div className="shell-side">
      {/* Brand */}
      <div style={{padding:"18px 16px 16px",borderBottom:"1px solid var(--border)",display:"flex",alignItems:"center",gap:10}}>
        <svg width="22" height="22" viewBox="0 0 96 96">
          <circle cx="48" cy="48" r="20" fill="none" stroke="#6DF7B5" strokeDasharray="2 3" opacity="0.6"/>
          <circle cx="48" cy="48" r="12" fill="none" stroke="#6DF7B5"/>
          <circle cx="48" cy="48" r="4" fill="#6DF7B5"/>
        </svg>
        <div style={{lineHeight:1}}>
          <div style={{fontFamily:"var(--font-display)",fontSize:14,fontWeight:600,letterSpacing:"-0.01em",color:"var(--bone-100)"}}>
            LOST IN THE MES<span style={{fontFamily:"var(--font-mono)",fontSize:11,color:"var(--signal-300)",verticalAlign:"2px"}}>(h)</span>S
          </div>
          <div className="stamp" style={{fontSize:9,marginTop:4}}>MESH CONSOLE · v0.4.2</div>
        </div>
      </div>

      {/* Nav */}
      <div style={{padding:"10px 0 6px"}}>
        <Stamp style={{padding:"4px 16px 8px",fontSize:9}}>MESH</Stamp>
        {items.map(item => (
          <div key={item.id}
               className={`nav-item ${screen === item.id ? "active" : ""}`}
               onClick={() => setScreen(item.id)}>
            <span style={{display:"inline-flex"}}>{item.icon}</span>
            <span>{item.label}</span>
            {item.count !== null && <span className="nav-count">{item.count}</span>}
          </div>
        ))}
      </div>

      {/* Mesh health summary at bottom of rail */}
      <div style={{marginTop:"auto",padding:"14px 16px",borderTop:"1px solid var(--border)"}}>
        <Stamp style={{fontSize:9,marginBottom:10}}>MESH HEALTH</Stamp>
        <div style={{display:"flex",alignItems:"baseline",justifyContent:"space-between",marginBottom:10}}>
          <div className="ticker" style={{fontSize:28,color:"var(--signal-300)",fontWeight:500}}>{avgPrr}</div>
          <div className="stamp" style={{fontSize:9}}>PRR.AVG</div>
        </div>
        <div style={{display:"flex",flexDirection:"column",gap:6}}>
          <div style={{display:"flex",justifyContent:"space-between",alignItems:"center",fontFamily:"var(--font-mono)",fontSize:11}}>
            <span style={{color:"var(--signal-300)"}}>● OK</span>
            <span style={{color:"var(--bone-200)"}}>{ok}</span>
          </div>
          <div style={{display:"flex",justifyContent:"space-between",alignItems:"center",fontFamily:"var(--font-mono)",fontSize:11}}>
            <span style={{color:"var(--uplink-300)"}}>● RELAY</span>
            <span style={{color:"var(--bone-200)"}}>{relay}</span>
          </div>
          <div style={{display:"flex",justifyContent:"space-between",alignItems:"center",fontFamily:"var(--font-mono)",fontSize:11}}>
            <span style={{color:"var(--lost-300)"}}>● LOST</span>
            <span style={{color:"var(--bone-200)"}}>{lost}</span>
          </div>
        </div>
      </div>
    </div>
  );
}

window.Sidebar = Sidebar;
