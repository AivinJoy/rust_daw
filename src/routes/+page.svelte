<!-- src\routes\+page.svelte -->

<script lang="ts">
  import LandingModal from '$lib/components/LandingModal.svelte';
  import TrackList from '$lib/components/TrackList.svelte';
  import Header from '$lib/components/Header.svelte';
  // Import new TopToolbar
  import TopToolbar from '$lib/components/TopToolbar.svelte';
  import Timeline from '$lib/components/Timeline.svelte';

  let view: 'landing' | 'studio' = $state('landing');
  let showModal = $state(false); 
  
  // Start with the default voice track shown in the image
  let tracks = $state([
    { id: 1, name: 'Voice/Audio', color: 'bg-brand-blue' },
  ]);

  function handleInitialSelection(event: CustomEvent<string>) {
    // If they chose something specific initially, add it, otherwise just show the default
    if (tracks.length === 0) {
         addNewTrack(event.detail);
    }
    view = 'studio';
  }

  function handleAddRequest() { showModal = true; }

  function handleModalSelection(event: CustomEvent<string>) {
    addNewTrack(event.detail);
    showModal = false;
  }

  // In src/routes/+page.svelte

  function addNewTrack(type: string) {
    const id = tracks.length + 1;
    
    // Define a palette of colors
    const colors = [
        'bg-brand-blue', 
        'bg-brand-red', 
        'bg-purple-500', 
        'bg-emerald-500', 
        'bg-orange-500', 
        'bg-pink-500'
    ];
    
    // Pick a color based on the track ID (cycles through the list)
    const color = colors[(id - 1) % colors.length];
    
    const name = type === 'record' ? `Recording ${id}` : `Imported Audio ${id}`;
    tracks = [...tracks, { id, name, color }];
  }
</script>

<main class="h-screen w-screen bg-[#0f0f16] text-white overflow-hidden relative font-sans flex flex-col">
  
  {#if view === 'landing' || showModal}
    <div class="absolute inset-0 z-50">
        <LandingModal on:select={view === 'landing' ? handleInitialSelection : handleModalSelection} />
    </div>
  {/if}

  {#if view === 'studio'}
    <Header/>
    <TopToolbar />

    <div class="flex-1 flex overflow-hidden relative">
        
        <TrackList {tracks} on:requestAdd={handleAddRequest} />

        <Timeline {tracks} /> 

    </div>

    {/if}

</main>