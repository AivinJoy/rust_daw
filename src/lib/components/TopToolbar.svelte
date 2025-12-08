<!-- src\lib\components\TopToolbar.svelte -->

<script lang="ts">
  import { 
    Menu, Undo, Redo, Timer, Play, Square, Circle, Repeat, 
    SlidersHorizontal, Volume2, Save, Share, Bell, ChevronDown 
  } from 'lucide-svelte';

  let isPlaying = $state(false);
  let isRecording = $state(false);
  let bpm = $state(120);
  let timeSignature = $state('4 / 4');
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

    <button class="h-10 px-4 rounded-lg bg-white/5 border border-white/10 flex items-center gap-2 text-sm text-white/60 hover:bg-white/10">
        Key <ChevronDown size={14} />
     </button>
  </div>

  <div class="flex items-center gap-6 flex-1 justify-center">
    
    <div class="flex items-center gap-2">
        <button onclick={() => isPlaying = !isPlaying} class={`w-10 h-10 rounded-full flex items-center justify-center transition-all ${isPlaying ? 'bg-brand-blue text-white shadow-lg shadow-brand-blue/50' : 'bg-white/10 hover:bg-white/20'}`}>
            {#if isPlaying} <Square size={16} class="fill-current" /> {:else} <Play size={18} class="fill-current ml-0.5" /> {/if}
        </button>
        <button onclick={() => isRecording = !isRecording} class={`w-10 h-10 rounded-full flex items-center justify-center transition-all ${isRecording ? 'bg-brand-red text-white shadow-lg shadow-brand-red/50' : 'bg-white/10 hover:bg-white/20 text-brand-red'}`}>
             <Circle size={16} class="fill-current" />
        </button>
         <button class="text-white/40 hover:text-white p-2"><Repeat size={18} /></button>
    </div>

    <div class="bg-black/30 border border-white/10 rounded-lg px-6 py-2 font-mono text-xl tracking-wider text-white/90 shadow-inner">
        00:00.0
    </div>

  </div>


  <div class="flex items-center gap-4 justify-end flex-1">
    
    <button class="h-9 px-3 rounded-lg bg-brand-purple/10 border border-brand-purple/30 flex items-center gap-2 text-sm text-brand-purple hover:bg-brand-purple/20 transition-all">
        <SlidersHorizontal size={14} /> Mastering <ChevronDown size={14} />
    </button>

    <div class="flex items-center gap-2 mx-2">
        <Volume2 size={16} class="text-white/40" />
        <input type="range" class="w-24 h-1 bg-white/10 rounded-full appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white" />
    </div>

    <div class="h-6 w-px bg-white/10 mx-2"></div>

    <button class="h-9 px-4 rounded-lg border border-white/10 flex items-center gap-2 text-sm hover:bg-white/5 transition-all">
        <Save size={16} /> Save
    </button>
    
    <button class="h-9 px-4 rounded-lg bg-brand-blue flex items-center gap-2 text-sm font-medium text-white hover:bg-brand-blue/80 transition-all shadow-lg shadow-brand-blue/20">
        <Share size={16} /> Export
    </button>

     <button class="text-white/40 hover:text-white p-2 relative">
        <Bell size={18} />
        <div class="absolute top-1.5 right-1.5 w-2 h-2 bg-brand-red rounded-full border border-[#0f0f16]"></div>
    </button>

  </div>
</div>