<script>
  export let neighbors;

  function prrColor(prr) {
    if (prr > 0.8) return 'text-[#00ff41]';
    if (prr > 0.5) return 'text-[#ffb100]';
    return 'text-[#ff3e3e]';
  }
</script>

<div class="space-y-2">
  {#if neighbors.length === 0}
    <div class="text-sm opacity-40 italic py-10 text-center">Searching for mesh neighbors...</div>
  {:else}
    <div class="grid grid-cols-4 text-[10px] uppercase opacity-40 mb-2 px-2">
      <span>ID</span>
      <span>PRR</span>
      <span>Signal</span>
      <span>Last Seen</span>
    </div>
    {#each neighbors as node}
      <div class="flex items-center justify-between p-2 border border-[#00ff41]/10 hover:bg-[#00ff41]/5 transition-colors">
        <div class="flex items-center space-x-3 w-full">
          <span class="font-bold w-1/4">ID_{node.id.toString().padStart(3, '0')}</span>
          <span class={`font-mono w-1/4 ${prrColor(node.prr)}`}>{(node.prr * 100).toFixed(1)}%</span>
          <span class="w-1/4 opacity-70">{node.rssi_dbm} dBm</span>
          <span class="w-1/4 text-right opacity-50 tabular-nums">PRR {(node.prr * 100).toFixed(0)}%</span>
        </div>
      </div>
    {/each}
  {/if}
</div>
