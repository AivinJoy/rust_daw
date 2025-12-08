<script lang="ts">
  import { Magnet, ZoomIn, ZoomOut, Plus } from 'lucide-svelte';
  // IMPORT THE WAVEFORM COMPONENT
  import WaveformClip from './WaveformClip.svelte';

  let { tracks = [] } = $props();

  // 1. ZOOM STATE
  let pixelsPerBar = $state(96); 
  
  // 2. SCROLLABLE AREA
  let totalBars = 100;
  let measures = Array.from({ length: totalBars }, (_, i) => i + 1);

  // Zoom Logic
  function zoomIn() {
    pixelsPerBar = Math.min(pixelsPerBar * 1.5, 300);
  }
  function zoomOut() {
    pixelsPerBar = Math.max(pixelsPerBar / 1.5, 48);
  }
</script>

<div class="flex-1 h-full relative flex flex-col bg-[#13131f]/90 backdrop-blur-md overflow-hidden">
  
  <div class="h-8 flex border-b border-white/10 bg-[#1a1a2e] shrink-0 z-20">
    
    <div class="flex-1 flex items-end overflow-hidden relative pb-1">
        {#each measures as measure}
            <div 
                class="shrink-0 h-full border-l border-white/5 text-[10px] text-white/40 pl-1 flex items-end pb-1 font-mono select-none relative"
                style="width: {pixelsPerBar}px;"
            >
                {measure}
                
                <div class="absolute bottom-0 left-0 right-0 h-1/2 flex justify-evenly items-end pointer-events-none">
                    {#if pixelsPerBar > 60}
                        <div class="h-1.5 w-px bg-white/10"></div>
                        <div class="h-2 w-px bg-white/20"></div>
                        <div class="h-1.5 w-px bg-white/10"></div>
                    {/if}
                </div>
            </div>
        {/each}
        
        <div class="absolute top-0 left-0 h-1 bg-red-500/50 rounded-b-sm" style="width: {pixelsPerBar * 4}px"></div>
    </div>
    
    <div class="flex items-center border-l border-white/10 px-1 bg-[#151520]">
        <button class="p-1.5 text-white/40 hover:text-white hover:bg-white/5 rounded"><Magnet size={14} /></button>
        <button onclick={zoomOut} class="p-1.5 text-white/40 hover:text-white hover:bg-white/5 rounded"><ZoomOut size={14} /></button>
        <button onclick={zoomIn} class="p-1.5 text-white/40 hover:text-white hover:bg-white/5 rounded"><ZoomIn size={14} /></button>
    </div>
  </div>

  <div class="flex-1 relative overflow-auto custom-scrollbar">
    
    <div class="relative" style="width: {totalBars * pixelsPerBar}px; min-height: 100%;">

        <div class="absolute inset-0 flex flex-col pt-4 px-0 pointer-events-none"> 
            {#each tracks as track}
                <div class="w-full h-24 mb-2 relative border-b border-white/5 flex items-center px-1">
                    
                    <div 
                        class={`absolute inset-0 transition-colors duration-300 ${track.color}`} 
                        style="opacity: 0.15;" 
                    ></div>

                    <div class="relative h-full flex items-center" style="width: {pixelsPerBar * 4}px;">
                         <WaveformClip 
                            color={track.color} 
                            width={100} 
                        />
                    </div>

                </div>
            {/each}
        </div>

        <div class="absolute inset-0 flex pointer-events-none h-full">
            {#each measures as measure}
                <div class="shrink-0 h-full border-l border-white/5" style="width: {pixelsPerBar}px;">
                     {#if pixelsPerBar > 60}
                        <div class="w-full h-full flex justify-evenly">
                            <div class="h-full w-px bg-white/5"></div>
                            <div class="h-full w-px bg-white/5"></div>
                            <div class="h-full w-px bg-white/5"></div>
                        </div>
                     {/if}
                </div>
            {/each}
        </div>

        {#if tracks.length === 0}
            <div class="sticky left-0 right-0 top-0 h-[300px] flex items-center justify-center pointer-events-none z-10 mt-10">
                <div class="w-[500px] h-[120px] border-2 border-dashed border-white/10 rounded-xl bg-white/5 flex flex-col items-center justify-center gap-3 backdrop-blur-sm">
                    <div class="p-2 rounded-full bg-white/10">
                        <Plus size={24} class="text-white/50" />
                    </div>
                    <span class="text-white/40 text-sm font-light tracking-wide">Drop a loop or an audio/MIDI/video file</span>
                </div>
            </div>
        {/if}

        <div class="absolute top-0 bottom-0 left-0 w-px bg-white z-30 shadow-[0_0_10px_rgba(255,255,255,0.5)] pointer-events-none">
            <div class="absolute -top-1 -left-[3px] w-2 h-2 bg-white rounded-full"></div>
        </div>

    </div>
  </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar {
        width: 10px;
        height: 10px;
    }
    .custom-scrollbar::-webkit-scrollbar-track {
        background: #0f0f16;
        border-left: 1px solid rgba(255,255,255,0.05);
    }
    .custom-scrollbar::-webkit-scrollbar-thumb {
        background: rgba(255,255,255,0.1);
        border-radius: 5px;
        border: 2px solid #0f0f16;
    }
    .custom-scrollbar::-webkit-scrollbar-thumb:hover {
        background: rgba(255,255,255,0.2);
    }
</style>