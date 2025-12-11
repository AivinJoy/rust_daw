<script lang="ts">
  import { 
    Menu, Undo, Redo, Timer, Play, Pause, Circle, 
    SlidersHorizontal, Volume2, Save, Share, SkipBack, ChevronDown 
  } from 'lucide-svelte';
  import { createEventDispatcher } from 'svelte';

  // 1. Accept Props
  // 'bpm' is bindable so changes here update the Timeline grid immediately
  let { 
    isPlaying = false, 
    isRecording = false, 
    currentTime = 0, 
    bpm = $bindable(120) 
  } = $props();

  const dispatch = createEventDispatcher();

  // Local state for UI-only elements
  let timeSignature = $state('4 / 4');

  function togglePlay() {
    if (isPlaying) {
      dispatch('pause');
    } else {
      dispatch('play');
    }
  }

  function toggleRecord() {
    dispatch('record');
  }

  function returnToStart() {
    dispatch('rewind'); 
  }

  function formatTimeDisplay(totalSeconds: number) {
      const m = Math.floor(totalSeconds / 60);
      const s = Math.floor(totalSeconds % 60);
      const ms = Math.floor((totalSeconds % 1) * 10); 
      return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}.${ms}`;
  }
</script>

<div class="h-16 w-full bg-[#0f0f16] border-b border-white/10 flex items-center justify-between px-4 relative z-50">
  
  <div class="flex items-center gap-4">
    <button class="text-white/60 hover:text-white"><Menu size={20} /></button>
    <div class="h-6 w-px bg-white/10 mx-2"></div>
    <div class="flex items-center gap-1 bg-white/5 rounded-lg p-1">
        <button class="p-1.5 text-white/40 hover:text-white rounded"><Undo size={16} /></button>
        <button class="p-1.5 text-white/40 hover:text-white rounded"><Redo size={16} /></button>
    </div>
  </div>

  <div class="flex items-center gap-3 flex-1 justify-center ml-8">
     <button class="w-10 h-10 rounded-lg bg-white/5 border border-white/10 flex items-center justify-center text-white/60 hover:text-brand-blue hover:border-brand-blue/50 transition-all">
        <Timer size={18} />
    </button>

    <div class="flex items-center bg-white/5 border border-white/10 rounded-lg h-10 px-3 gap-2">
        <input type="number" bind:value={bpm} class="bg-transparent w-12 text-center font-mono text-sm focus:outline-none" />
        <span class="text-xs text-white/40">bpm</span>
        <div class="h-4 w-px bg-white/10"></div>
        <input type="text" bind:value={timeSignature} class="bg-transparent w-12 text-center font-mono text-sm focus:outline-none" />
    </div>
  </div>

  <div class="flex items-center gap-6 flex-1 justify-center">
    
    <div class="flex items-center gap-2">
        <button 
            onclick={returnToStart}
            class="w-10 h-10 rounded-full flex items-center justify-center bg-white/5 hover:bg-white/10 text-white/60 hover:text-white transition-all active:scale-95"
            title="Return to Start"
        >
            <SkipBack size={16} class="fill-current" />
        </button>

        <button 
          onclick={togglePlay} 
          class={`w-10 h-10 rounded-full flex items-center justify-center transition-all ${isPlaying ? 'bg-brand-blue text-white shadow-lg shadow-brand-blue/50' : 'bg-white/5 hover:bg-white/10 text-white'}`}
        >
            {#if isPlaying} 
              <Pause size={16} class="fill-current" /> 
            {:else} 
              <Play size={16} class="fill-current ml-0.5" /> 
            {/if}
        </button>
        
        <button 
          onclick={toggleRecord} 
          class={`w-10 h-10 rounded-full flex items-center justify-center transition-all ${isRecording ? 'bg-brand-red text-white shadow-lg shadow-brand-red/50 animate-pulse' : 'bg-white/5 hover:bg-white/10 text-brand-red'}`}
        >
             <Circle size={14} class="fill-current" />
        </button>
    </div>

    <div class="bg-black/30 border border-white/10 rounded-lg px-4 py-2 font-mono text-xl tracking-wider text-white/90 shadow-[inset_0_2px_4px_rgba(0,0,0,0.5)] w-32 text-center">
        {formatTimeDisplay(currentTime)}
    </div>

  </div>

  <div class="flex items-center gap-4 justify-end flex-1">
    
    <button class="h-9 px-3 rounded-lg bg-brand-purple/10 border border-brand-purple/30 flex items-center gap-2 text-sm text-brand-purple hover:bg-brand-purple/20 transition-all">
        <SlidersHorizontal size={14} /> Mastering
    </button>

    <div class="flex items-center gap-2 mx-2">
        <Volume2 size={16} class="text-white/40" />
        <input type="range" class="w-24 h-1 bg-white/10 rounded-full appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white" />
    </div>

    <div class="h-6 w-px bg-white/10 mx-2"></div>
    
    <button class="h-9 px-4 rounded-lg bg-brand-blue flex items-center gap-2 text-sm font-medium text-white hover:bg-brand-blue/80 transition-all shadow-lg shadow-brand-blue/20">
        <Share size={16} /> Export
    </button>

  </div>
</div>