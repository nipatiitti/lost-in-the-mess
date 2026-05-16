<script>
  export let data;
  
  $: neighborCount = data.neighbors.length;
  $: msgCount = data.messages.length;
</script>

<div class="glass-card p-4 space-y-4">
  <h2 class="text-sm font-bold uppercase opacity-70">System Metrics</h2>
  
  <div class="grid grid-cols-2 gap-4">
    <div class="border border-[#00ff41]/20 p-3">
      <div class="text-[10px] opacity-50 uppercase">Peers</div>
      <div class="text-2xl font-bold">{neighborCount}</div>
    </div>
    <div class="border border-[#00ff41]/20 p-3">
      <div class="text-[10px] opacity-50 uppercase">Objects</div>
      <div class="text-2xl font-bold">{msgCount}</div>
    </div>
  </div>

  <div class="space-y-2">
    <div class="flex justify-between text-[10px] uppercase opacity-50">
      <span>Radio Activity</span>
      <span>{neighborCount > 0 ? 'NOMINAL' : 'IDLE'}</span>
    </div>
    <div class="h-1 bg-[#00ff41]/10 w-full overflow-hidden">
      {#if neighborCount > 0}
        <div class="h-full bg-[#00ff41] animate-[progress_2s_infinite_linear]" style="width: 40%"></div>
      {/if}
    </div>
  </div>
</div>

<style>
  @keyframes progress {
    0% { transform: translateX(-100%); }
    100% { transform: translateX(300%); }
  }
</style>
