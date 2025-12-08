<!-- src\lib\components\TrackList.svelte -->

<script lang="ts">
  import { Plus } from 'lucide-svelte';
  import TrackControl from './TrackControl.svelte';
  import { createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher();

  // Receive tracks prop
  let { tracks = [] } = $props();

  function requestAddTrack() {
    dispatch('requestAdd');
  }
</script>

<div class="w-[320px] h-full flex flex-col border-r border-white/10 bg-[#0a0a0f]/60 backdrop-blur-xl relative z-10">
  
  <div class="h-8 flex items-center justify-between px-4 border-b border-white/5 shadow-[0_4px_20px_rgba(0,0,0,0.2)]">
    <span class="text-xs font-bold tracking-widest text-white/60 glow-text-blue">TRACKS</span>
    
    <button 
        onclick={requestAddTrack}
        class="h-6 px-3 rounded-full bg-brand-blue/10 border border-brand-blue/30 flex items-center gap-1.5 text-brand-blue hover:bg-brand-blue/20 hover:border-brand-blue/60 transition-all shadow-[0_0_10px_rgba(59,130,246,0.2)] group"
    >
        <Plus size={13} class="group-hover:scale-110 transition-transform" />
        <span class="text-[10px] font-bold uppercase tracking-wider">Add Track</span>
    </button>
  </div>

  <div class="flex-1 overflow-y-auto p-4 scrollbar-hide space-y-4">
    {#each tracks as track (track.id)}
        <TrackControl id={track.id} name={track.name} color={track.color} />
    {/each}
  </div>
</div>

<style>
    .scrollbar-hide::-webkit-scrollbar { display: none; }
    .scrollbar-hide { -ms-overflow-style: none; scrollbar-width: none; }
    .glow-text-blue { text-shadow: 0 0 10px rgba(59, 130, 246, 0.5); }
</style>