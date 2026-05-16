/* UplinkApps.jsx — applications mounted on top of the mesh */

function UplinkApps({ onLaunch }) {
  const apps = [
    { id:"image", icon: I.image,  name:"Image relay",  v:"0.4", note:"Chunked JPEG/PNG over lossy hops. Falls back to thumbnail @ PRR < 0.3.", state:"on" },
    { id:"text",  icon: I.message,name:"Text channel", v:"1.0", note:"Reliable short messages. Always delivers.", state:"on" },
    { id:"video", icon: I.video,  name:"Video stream", v:"0.2", note:"Low-FPS forward over strongest 3-hop path.", state:"degrading" },
    { id:"file",  icon: I.file,   name:"File transfer",v:"0.1", note:"Generic blob, parity-coded. Pending review.", state:"off" },
  ];
  return (
    <div style={{padding:"20px 24px",overflow:"auto",flex:1}}>
      <div style={{display:"flex",justifyContent:"space-between",alignItems:"flex-end",marginBottom:18}}>
        <div>
          <div className="stamp" style={{fontSize:10}}>05 / APPLICATIONS</div>
          <div style={{fontFamily:"var(--font-display)",fontSize:24,letterSpacing:"-0.01em",color:"var(--bone-100)",marginTop:4}}>
            {apps.length} apps mounted on transport
          </div>
        </div>
        <Button variant="secondary">{I.plus}<span>ADD APP</span></Button>
      </div>
      <div style={{display:"grid",gridTemplateColumns:"repeat(auto-fill,minmax(280px,1fr))",gap:12}}>
        {apps.map(a => (
          <div key={a.id} onClick={() => onLaunch && onLaunch(a.id)}
            style={{background:"var(--bg-panel)",border:"1px solid var(--border)",borderRadius:4,padding:16,cursor:"pointer",transition:"background 120ms"}}
            onMouseEnter={(e)=>e.currentTarget.style.background="var(--ink-200)"}
            onMouseLeave={(e)=>e.currentTarget.style.background="var(--bg-panel)"}>
            <div style={{display:"flex",alignItems:"center",justifyContent:"space-between",marginBottom:14}}>
              <div style={{display:"flex",alignItems:"center",gap:12}}>
                <div style={{width:32,height:32,border:`1px solid ${a.state === "off" ? "var(--border)" : "var(--signal-300)"}`,borderRadius:2,display:"flex",alignItems:"center",justifyContent:"center",background:a.state === "off" ? "transparent" : "rgba(109,247,181,0.08)",color:a.state === "off" ? "var(--bone-400)" : "var(--signal-300)"}}>
                  {a.icon}
                </div>
                <div>
                  <div style={{fontFamily:"var(--font-display)",fontSize:15,color:"var(--bone-100)",fontWeight:500}}>{a.name}</div>
                  <div className="stamp" style={{fontSize:9,marginTop:2}}>v{a.v} · MESH-APP</div>
                </div>
              </div>
              {a.state === "on" && <Chip tone="ok">ENABLED</Chip>}
              {a.state === "degrading" && <Chip tone="warm">DEGRADING</Chip>}
              {a.state === "off" && <Chip tone="idle">DISABLED</Chip>}
            </div>
            <div className="mono" style={{fontSize:11,color:"var(--bone-300)",lineHeight:1.5}}>{a.note}</div>
          </div>
        ))}
        {/* Add slot */}
        <div style={{background:"var(--bg-panel)",border:"1px dashed var(--border-strong)",borderRadius:4,padding:16,cursor:"pointer",display:"flex",alignItems:"center",justifyContent:"center",minHeight:144}}>
          <div style={{textAlign:"center",color:"var(--bone-400)"}}>
            {I.plus}
            <div className="stamp" style={{fontSize:10,marginTop:6}}>MOUNT APPLICATION</div>
          </div>
        </div>
      </div>
    </div>
  );
}

window.UplinkApps = UplinkApps;
