<script lang="ts">
  import { PenLine } from 'lucide-svelte';

  let projectName = $state("Untitled Project");
  let isEditing = $state(false);
  
  // FIX: Declare the element reference as a $state
  let inputRef = $state<HTMLInputElement>();

  function handleEdit() {
    isEditing = true;
    setTimeout(() => inputRef?.focus(), 0);
  }

  function handleBlur() {
    isEditing = false;
    if (projectName.trim() === "") projectName = "Untitled Project";
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === 'Enter') handleBlur();
  }
</script>

<div class="h-10 w-full bg-[#050508] border-b border-white/5 flex items-center justify-between px-4 select-none z-50 relative">
  
  <div class="flex items-center w-1/3">
    <h1 class="relative text-2xl font-extralight tracking-[0.2em] text-white/90 font-sans cursor-default">
        HAVEN
    </h1>
  </div>

  <div class="flex-1 flex justify-center w-1/3">
    <div class="relative group flex items-center justify-center">
        
        {#if isEditing}
            <input 
                bind:this={inputRef}
                type="text" 
                bind:value={projectName} 
                onblur={handleBlur}
                onkeydown={handleKey}
                class="bg-white/5 border border-brand-blue/50 rounded px-2 py-0.5 text-center text-sm font-medium text-white focus:outline-none focus:ring-1 focus:ring-brand-blue w-64"
            />
        {:else}
            <button 
                onclick={handleEdit}
                class="flex items-center gap-2 px-3 py-1 rounded hover:bg-white/5 transition-all text-white/60 hover:text-white"
            >
                <span class="text-sm font-medium tracking-wide">{projectName}</span>
                <PenLine size={12} class="opacity-0 group-hover:opacity-100 transition-opacity text-white/30" />
            </button>
        {/if}

    </div>
  </div>

  <div class="w-1/3 flex justify-end"></div>

</div>