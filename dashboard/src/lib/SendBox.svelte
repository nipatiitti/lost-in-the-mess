<script>
  let text = '';
  let sending = false;

  async function send() {
    if (!text.trim() || sending) return;
    sending = true;
    try {
      const res = await fetch('/api/send', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ text })
      });
      if (res.ok) {
        text = '';
      }
    } catch (e) {
      console.error(e);
    } finally {
      sending = false;
    }
  }

  function onKeydown(e) {
    if (e.key === 'Enter') send();
  }
</script>

<div class="glass-card p-4 space-y-4">
  <h2 class="text-sm font-bold uppercase opacity-70">Tactical Broadcast</h2>
  
  <div class="space-y-2">
    <input 
      type="text" 
      bind:value={text}
      on:keydown={onKeydown}
      placeholder="ENTER MESSAGE..."
      class="w-full bg-black border border-[#00ff41]/30 p-2 text-sm focus:outline-none focus:border-[#00ff41] transition-colors"
      disabled={sending}
    />
    
    <button 
      on:click={send}
      disabled={sending || !text.trim()}
      class="w-full bg-[#00ff41]/10 border border-[#00ff41] text-[#00ff41] p-2 text-xs font-bold uppercase hover:bg-[#00ff41] hover:text-black transition-all disabled:opacity-30 disabled:cursor-not-allowed"
    >
      {sending ? 'Transmitting...' : 'Broadcast Signal'}
    </button>
  </div>
</div>
