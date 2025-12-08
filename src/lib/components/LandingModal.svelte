<!--src\lib\components\LandingModal.svelte -->

<script lang="ts">
  import { Mic, Upload, Music } from 'lucide-svelte';
  import { createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher();

  function select(option: 'record' | 'upload') {
    dispatch('select', option);
  }
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-md">
  
  <div class="glass-panel w-[650px] p-12 text-center relative overflow-hidden group">
    
    <div class="absolute top-[-50%] left-[-50%] w-[200%] h-[200%] bg-linear-to-br from-purple-900/20 via-transparent to-blue-900/20 animate-spin-slow pointer-events-none"></div>

    <h1 class="relative text-5xl font-extralight tracking-[0.2em] mb-2 text-white/90 font-sans">HAVEN</h1>
    <p class="relative text-white/40 text-xs tracking-[0.3em] mb-12 uppercase">Audio Production Environment</p>
    
    <div class="relative grid grid-cols-2 gap-6">
      
      <button 
        onclick={() => select('record')}
        class="glass-panel p-10 flex flex-col items-center justify-center gap-6 border border-white/5 hover:border-red-500/40 hover:bg-red-500/5 transition-all duration-300 group/btn"
      >
        <div class="p-5 rounded-full bg-white/5 group-hover/btn:bg-red-500/20 transition-colors shadow-lg shadow-black/20">
            <Mic size={40} class="text-white/70 group-hover/btn:text-red-400 transition-colors" />
        </div>
        <span class="text-white/50 text-sm tracking-widest font-light group-hover/btn:text-red-400 transition-colors">RECORD</span>
      </button>

      <button 
        onclick={() => select('upload')}
        class="glass-panel p-10 flex flex-col items-center justify-center gap-6 border border-white/5 hover:border-blue-500/40 hover:bg-blue-500/5 transition-all duration-300 group/btn"
      >
        <div class="p-5 rounded-full bg-white/5 group-hover/btn:bg-blue-500/20 transition-colors shadow-lg shadow-black/20">
            <Upload size={40} class="text-white/70 group-hover/btn:text-blue-400 transition-colors" />
        </div>
        <span class="text-white/50 text-sm tracking-widest font-light group-hover/btn:text-blue-400 transition-colors">IMPORT</span>
      </button>

    </div>

    <div class="mt-10 flex justify-center gap-3 text-white/10 items-center">
        <div class="h-px w-8 bg-white/10"></div>
        <Music size={14} />
        <div class="h-px w-8 bg-white/10"></div>
    </div>

  </div>
</div>

<style>
    @keyframes spin-slow {
        from { transform: rotate(0deg); }
        to { transform: rotate(360deg); }
    }
    .animate-spin-slow {
        animation: spin-slow 25s linear infinite;
    }
</style>