<script>
  export let messages;
  
  function formatTime(ts) {
    return new Date(ts * 1000).toLocaleTimeString([], { hour12: false });
  }
</script>

<div class="h-64 overflow-y-auto space-y-1 font-mono text-xs">
  {#if messages.length === 0}
    <div class="text-center opacity-30 py-20 uppercase tracking-widest">No traffic detected</div>
  {:else}
    {#each [...messages].reverse() as msg}
      <div class="p-1 border-l-2 border-[#00ff41]/40 bg-[#00ff41]/5">
        <span class="opacity-40">[{formatTime(msg.timestamp)}]</span>
        <span class="text-[#ffb100] font-bold"> SRC_{msg.source} </span>
        <span class="text-[#00ff41]"> >> {msg.text}</span>
      </div>
    {/each}
  {/if}
</div>
