<script>
  export let screen;
  export let setScreen;
  export let nodesCount = 0;

  $: tabs = [
    { id: "topology", label: "Topology" },
    { id: "nodes",    label: `Nodes · ${nodesCount}` },
    { id: "uplink",   label: "Uplink" },
    { id: "log",      label: "Log" },
  ];
</script>

<div class="tabbar">
  {#each tabs as t}
    <button class="tab {screen === t.id ? 'active' : ''}" on:click={() => setScreen(t.id)}>
      {t.label}
    </button>
  {/each}
</div>
