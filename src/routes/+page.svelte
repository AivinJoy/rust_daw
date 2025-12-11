<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onDestroy } from 'svelte';

  import LandingModal from '$lib/components/LandingModal.svelte';
  import TrackList from '$lib/components/TrackList.svelte';
  import Header from '$lib/components/Header.svelte';
  import TopToolbar from '$lib/components/TopToolbar.svelte';
  import Timeline from '$lib/components/Timeline.svelte';

  // --- STATE ---
  let view: 'landing' | 'studio' = $state('landing');
  let showModal = $state(false); 
  let isPlaying = $state(false);
  let currentTime = $state(0); 
  
  // --- GLOBAL BPM STATE ---
  // Defaults to 120, but updates automatically on import
  let bpm = $state(120); 
  $effect(() => {
      // Don't sync invalid values (0 or NaN)
      if (bpm > 0) {
          invoke('set_bpm', { bpm: bpm }).catch(e => console.error("Failed to set BPM:", e));
      }
  });

  let animationFrameId: number;

  // --- TYPE DEFINITION ---
  type Track = {
    id: number;
    name: string;
    color: string;
    duration?: number;
    startTime?: number; 
    waveform?: { mins: number[], maxs: number[] };
  };

  // --- TRACKS STATE ---
  let tracks = $state<Track[]>([
    { id: 1, name: 'Voice/Audio', color: 'bg-brand-blue' },
  ]);

  // --- HANDLERS ---
  async function handleInitialSelection(event: CustomEvent<string>) {
    await addNewTrack(event.detail);
    view = 'studio';
  }

  function handleAddRequest() { showModal = true; }

  async function handleModalSelection(event: CustomEvent<string>) {
    await addNewTrack(event.detail);
    showModal = false;
  }

  // --- CORE LOGIC: ADD TRACK + BACKEND IMPORT ---
  async function addNewTrack(type: string) {
    const id = tracks.length + 1;
    
    const colors = [
        'bg-brand-blue', 'bg-brand-red', 'bg-purple-500', 
        'bg-emerald-500', 'bg-orange-500', 'bg-pink-500'
    ];
    const color = colors[(id - 1) % colors.length];

    if (type === 'import' || type === 'upload') {
        try {
            // 1. Open System Dialog
            const selected = await open({
                multiple: false,
                filters: [{ name: 'Audio', extensions: ['wav', 'mp3', 'flac'] }]
            });

            if (selected && typeof selected === 'string') {
                // 2. Call Rust Backend (Now returns BPM too)
                const result = await invoke<{
                    mins: number[], 
                    maxs: number[], 
                    duration: number, 
                    bpm?: number 
                }>('import_track', { path: selected });
                
                const name = selected.split(/[\\/]/).pop() || `Imported ${id}`;

                // 3. AUTO-SET BPM 
                // If the backend found a BPM, update the global state immediately.
                if (result.bpm && result.bpm > 0) {
                    console.log("Detected BPM:", result.bpm);
                    bpm = Math.round(result.bpm); // Round to nearest whole number for cleaner grid
                }
                
                // 4. Update State
                tracks = [...tracks, { 
                    id, 
                    name, 
                    color, 
                    duration: result.duration, 
                    startTime: 0, 
                    waveform: { mins: result.mins, maxs: result.maxs } 
                }];
            }
        } catch (e) {
            console.error("Import failed:", e);
        }
    } 
    else if (type === 'record') {
        const name = `Recording ${id}`;
        tracks = [...tracks, { id, name, color, startTime: 0 }];
    } 
    else {
        const name = `Track ${id}`;
        tracks = [...tracks, { id, name, color, startTime: 0 }];
    }
  }

  // --- PLAYBACK LOOP ---
  async function togglePlayback() {
      if (isPlaying) {
          await invoke('pause');
          isPlaying = false;
          cancelAnimationFrame(animationFrameId);
      } else {
          await invoke('play');
          isPlaying = true;
          pollPosition();
      }
  }

  function pollPosition() {
      invoke<number>('get_position')
          .then((pos) => {
              currentTime = pos;
              if (isPlaying) {
                  animationFrameId = requestAnimationFrame(pollPosition);
              }
          })
          .catch(console.error);
  }

  // Basic Rewind Logic
  async function rewind() {
      currentTime = 0;
      if (isPlaying) {
          await invoke('pause');
          isPlaying = false;
          cancelAnimationFrame(animationFrameId);
      }
  }

  function handleKeydown(e: KeyboardEvent) {
      if (e.code === 'Space' && view === 'studio') {
          e.preventDefault();
          togglePlayback();
      }
  }

  onDestroy(() => {
    cancelAnimationFrame(animationFrameId);
  });
</script>

<svelte:window on:keydown={handleKeydown} />

<main class="h-screen w-screen bg-[#0f0f16] text-white overflow-hidden relative font-sans flex flex-col">
  
  {#if view === 'landing' || showModal}
    <div class="absolute inset-0 z-50">
        <LandingModal on:select={view === 'landing' ? handleInitialSelection : handleModalSelection} />
    </div>
  {/if}

  {#if view === 'studio'}
    <Header/>
    
    <TopToolbar 
        isPlaying={isPlaying} 
        currentTime={currentTime}
        bind:bpm={bpm} 
        on:play={togglePlayback} 
        on:pause={togglePlayback}
        on:rewind={rewind} 
    />

    <div class="flex-1 flex overflow-hidden relative">
        <TrackList {tracks} on:requestAdd={handleAddRequest} />
        
        <Timeline {tracks} currentTime={currentTime} bpm={bpm} /> 

    </div>

  {/if}

</main>