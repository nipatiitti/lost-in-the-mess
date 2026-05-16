/* PacketLog.jsx — mono table of packets */

function PacketLog({ entries, dense = false }) {
  return (
    <div style={{flex:1,display:"flex",flexDirection:"column",padding:dense ? 0 : "20px 24px",overflow:"hidden"}}>
      {!dense && (
        <div style={{marginBottom:14}}>
          <div className="stamp" style={{fontSize:10}}>04 / PACKET LOG</div>
          <div style={{fontFamily:"var(--font-display)",fontSize:24,letterSpacing:"-0.01em",color:"var(--bone-100)",marginTop:4}}>
            {entries.length} packets · last 60s
          </div>
        </div>
      )}
      <div style={{flex:1,overflow:"auto",background:"var(--bg-panel)",border:"1px solid var(--border)",borderRadius:2}}>
        <div style={{display:"grid",gridTemplateColumns:"90px 60px 70px 1fr 70px",padding:"8px 14px",background:"var(--ink-200)",color:"var(--bone-400)",letterSpacing:"0.14em",fontFamily:"var(--font-mono)",fontSize:9,textTransform:"uppercase",borderBottom:"1px solid var(--border)",position:"sticky",top:0,zIndex:1}}>
          <div>UTC</div><div>NODE</div><div>KIND</div><div>PAYLOAD</div><div style={{textAlign:"right"}}>STATE</div>
        </div>
        {entries.map((e, i) => (
          <div key={e.id || i} style={{display:"grid",gridTemplateColumns:"90px 60px 70px 1fr 70px",padding:"7px 14px",fontFamily:"var(--font-mono)",fontSize:11,borderBottom:"1px solid var(--ink-200)",alignItems:"center"}}>
            <div style={{color:"var(--bone-500)"}}>{e.time}</div>
            <div style={{color: colorForState(e.nodeState || "ok")}}>{e.node}</div>
            <div style={{color:"var(--bone-100)"}}>{e.kind}</div>
            <div style={{color:"var(--bone-300)",whiteSpace:"nowrap",overflow:"hidden",textOverflow:"ellipsis"}}>{e.payload}</div>
            <div style={{textAlign:"right",color: colorForState(e.result === "OK" ? "ok" : e.result === "RELAY" ? "relay" : "lost")}}>{e.result}</div>
          </div>
        ))}
      </div>
    </div>
  );
}

window.PacketLog = PacketLog;
