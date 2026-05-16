/* Composer.jsx — Send a thing through the mesh */

function Composer({ nodes, onSend, lastResult }) {
  const [target, setTarget] = React.useState("N-04");
  const [kind, setKind] = React.useState("text");
  const [text, setText] = React.useState("holding pattern, awaiting nav");

  const targetNode = nodes.find(n => n.id === target);
  const expectedPrr = targetNode ? targetNode.prr : 1;
  const willLikelyLand = expectedPrr >= 0.6;

  return (
    <div style={{padding:"20px 24px",overflow:"auto",flex:1,display:"grid",gridTemplateColumns:"1fr 360px",gap:20}}>
      <div>
        <div className="stamp" style={{fontSize:10}}>03 / UPLINK</div>
        <div style={{fontFamily:"var(--font-display)",fontSize:24,letterSpacing:"-0.01em",color:"var(--bone-100)",marginTop:4,marginBottom:18}}>
          Push a payload into the swarm
        </div>

        {/* Target picker */}
        <div style={{marginBottom:18}}>
          <div className="stamp" style={{fontSize:9,marginBottom:8}}>TARGET NODE</div>
          <div style={{display:"flex",flexWrap:"wrap",gap:6}}>
            {nodes.filter(n => n.id !== "HUB").map(n => {
              const active = target === n.id;
              const c = colorForState(n.state);
              return (
                <button key={n.id} onClick={() => setTarget(n.id)}
                  style={{
                    padding:"6px 10px",
                    border:`1px solid ${active ? c : "var(--border)"}`,
                    background: active ? `rgba(${rgbOfTone(chipToneForState(n.state))},0.08)` : "transparent",
                    color: active ? c : "var(--bone-200)",
                    fontFamily:"var(--font-mono)",fontSize:11,letterSpacing:"0.08em",
                    borderRadius:2,cursor:"pointer",
                    display:"flex",alignItems:"center",gap:6,
                  }}>
                  <span style={{width:6,height:6,borderRadius:"50%",background:c,boxShadow: active ? `0 0 6px ${c}` : "none"}}/>
                  {n.label}
                  <span style={{opacity:0.6}}>· {n.prr.toFixed(2)}</span>
                </button>
              );
            })}
          </div>
        </div>

        {/* Kind picker */}
        <div style={{marginBottom:18}}>
          <div className="stamp" style={{fontSize:9,marginBottom:8}}>APPLICATION</div>
          <div style={{display:"flex",gap:6}}>
            {[
              {id:"text", icon: I.message, label:"Text"},
              {id:"image",icon: I.image,   label:"Image"},
              {id:"video",icon: I.video,   label:"Video"},
              {id:"file", icon: I.file,    label:"File"},
            ].map(k => (
              <button key={k.id} onClick={() => setKind(k.id)}
                style={{
                  padding:"10px 14px",
                  border:`1px solid ${kind === k.id ? "var(--signal-300)" : "var(--border)"}`,
                  background: kind === k.id ? "rgba(109,247,181,0.08)" : "transparent",
                  color: kind === k.id ? "var(--signal-300)" : "var(--bone-200)",
                  fontFamily:"var(--font-mono)",fontSize:11,letterSpacing:"0.14em",textTransform:"uppercase",
                  borderRadius:2,cursor:"pointer",display:"flex",alignItems:"center",gap:8,
                }}>
                {k.icon}{k.label}
              </button>
            ))}
          </div>
        </div>

        {/* Payload editor */}
        <div style={{marginBottom:18}}>
          <div className="stamp" style={{fontSize:9,marginBottom:8}}>PAYLOAD</div>
          {kind === "text" && (
            <textarea value={text} onChange={(e) => setText(e.target.value)}
              style={{
                width:"100%",minHeight:88,background:"var(--ink-100)",border:"1px solid var(--border)",
                borderRadius:2,padding:"12px 14px",color:"var(--bone-100)",
                fontFamily:"var(--font-mono)",fontSize:13,outline:"none",resize:"vertical",
              }}/>
          )}
          {kind !== "text" && (
            <div style={{border:"1px dashed var(--border-strong)",borderRadius:2,padding:"32px 14px",textAlign:"center",color:"var(--bone-400)"}}>
              <div style={{fontFamily:"var(--font-mono)",fontSize:12,letterSpacing:"0.14em",textTransform:"uppercase"}}>DROP {kind.toUpperCase()} HERE</div>
              <div className="stamp" style={{fontSize:9,marginTop:6}}>OR CLICK TO BROWSE</div>
            </div>
          )}
        </div>

        <div style={{display:"flex",gap:10,alignItems:"center"}}>
          <Button variant="primary" onClick={() => onSend && onSend({ target, kind, text })}>
            {I.send}<span>SEND →</span>
          </Button>
          <Button variant="secondary">SCHEDULE</Button>
          {lastResult && (
            <div className="mono" style={{fontSize:11,color: lastResult.result === "OK" ? "var(--signal-300)" : lastResult.result === "RELAY" ? "var(--uplink-300)" : "var(--lost-300)"}}>
              ▸ {lastResult.message}
            </div>
          )}
        </div>
      </div>

      {/* Side: route preview */}
      <div style={{display:"flex",flexDirection:"column",gap:14}}>
        <Panel title="ROUTE PREVIEW" framed>
          <div style={{padding:"16px"}}>
            <div className="mono" style={{fontSize:12,color:"var(--bone-300)",marginBottom:14}}>
              HUB → {target === "N-04" ? target : `N-04 → ${target}`}
            </div>
            <div style={{display:"flex",justifyContent:"space-between",marginBottom:14}}>
              <div>
                <div className="stamp" style={{fontSize:9}}>EXPECTED PRR</div>
                <div className="ticker" style={{fontSize:22,color: colorForPrr(expectedPrr),fontWeight:500,marginTop:4}}>
                  {expectedPrr.toFixed(2)}
                </div>
              </div>
              <div>
                <div className="stamp" style={{fontSize:9}}>HOPS</div>
                <div className="ticker" style={{fontSize:22,color:"var(--bone-100)",fontWeight:500,marginTop:4}}>
                  {targetNode ? targetNode.hops : 0}
                </div>
              </div>
              <div>
                <div className="stamp" style={{fontSize:9}}>ETA</div>
                <div className="ticker" style={{fontSize:22,color:"var(--bone-100)",fontWeight:500,marginTop:4}}>
                  {(20 + (targetNode ? targetNode.hops * 35 : 0))}ms
                </div>
              </div>
            </div>
            <div style={{padding:"10px",border:"1px solid var(--border)",borderRadius:2,background:"var(--ink-050)"}}>
              <div className="stamp" style={{fontSize:9,marginBottom:6}}>FORECAST</div>
              <div className="mono" style={{fontSize:11,color: willLikelyLand ? "var(--signal-300)" : "var(--uplink-300)"}}>
                {willLikelyLand ? "PACKET WILL LIKELY LAND." : "DEGRADED LINK — RETRY EXPECTED."}
              </div>
            </div>
          </div>
        </Panel>
      </div>
    </div>
  );
}

window.Composer = Composer;
