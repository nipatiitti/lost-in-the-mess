/* StatusBar.jsx — sticky top bar */

function StatusBar({ time, lastEvent, onSend }) {
  return (
    <div className="shell-top">
      <div style={{display:"flex",alignItems:"center",gap:10}}>
        <span style={{color:"var(--signal-300)",fontFamily:"var(--font-mono)",fontSize:10,letterSpacing:"0.22em"}}>● LIVE</span>
        <span className="stamp" style={{fontSize:10}}>MESH.TOPO / N=14 / 2.4GHz</span>
      </div>
      <div style={{flex:1,display:"flex",alignItems:"center",gap:10,minWidth:0,overflow:"hidden",whiteSpace:"nowrap"}}>
        <div className="stamp" style={{fontSize:10,color:"var(--bone-400)",flexShrink:0}}>LAST EVENT</div>
        <div style={{fontFamily:"var(--font-mono)",fontSize:11,color:"var(--bone-200)",overflow:"hidden",textOverflow:"ellipsis"}}>{lastEvent}</div>
      </div>
      <div style={{display:"flex",alignItems:"center",gap:14}}>
        <div className="stamp" style={{fontSize:10,color:"var(--bone-300)"}}>UTC {time}</div>
        <div style={{width:1,height:18,background:"var(--border)"}}/>
        <IconButton title="Refresh">{I.refresh}</IconButton>
        <Button variant="primary" onClick={onSend}>{I.send}<span>SEND</span></Button>
      </div>
    </div>
  );
}

window.StatusBar = StatusBar;
