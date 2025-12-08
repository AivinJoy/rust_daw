<!-- src\lib\components\WaveformClip.svelte -->

<script lang="ts">
  import { onMount } from 'svelte';

  let { color = 'bg-brand-blue', width = 100 } = $props();

  let canvas: HTMLCanvasElement;
  let container: HTMLDivElement;
  let resizeObserver: ResizeObserver;

  // PALETTE: Semi-transparent backgrounds (Glass) vs. Dark Solid Waveforms
  const colorPalette: Record<string, { bg: string; wave: string }> = {
    'bg-brand-blue':  { bg: 'rgba(191, 219, 254, 0.4)', wave: '#172554' }, 
    'bg-brand-red':   { bg: 'rgba(254, 202, 202, 0.4)', wave: '#450a0a' },
    'bg-purple-500':  { bg: 'rgba(233, 213, 255, 0.4)', wave: '#3b0764' },
    'bg-emerald-500': { bg: 'rgba(167, 243, 208, 0.4)', wave: '#022c22' },
    'bg-orange-500':  { bg: 'rgba(254, 215, 170, 0.4)', wave: '#431407' },
    'bg-pink-500':    { bg: 'rgba(251, 207, 232, 0.4)', wave: '#500724' }
  };

  // 1. GENERATE DATA ONCE (So it doesn't "jitter" or change shape when zooming)
  const generateWaveform = (points: number) => {
    return Array.from({ length: points }, () => Math.pow(Math.random(), 3) * 0.9 + 0.1);
  };
  // We keep this stable so the shape is consistent
  const waveformData = generateWaveform(250); 

  // 2. THE DRAWING FUNCTION (Reusable)
  const draw = () => {
    if (!canvas || !container) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    // A. High DPI Re-Scaling (Crucial for Zoom sharpness)
    const dpr = window.devicePixelRatio || 1;
    const rect = container.getBoundingClientRect();
    
    // Resize internal resolution to match new display size
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    
    // Normalize coordinate system
    ctx.scale(dpr, dpr);

    // B. Drawing Logic
    const w = rect.width;
    const h = rect.height;
    const centerY = h / 2;
    // Stretch the fixed data across the new width
    const step = w / (waveformData.length - 1);

    const colors = colorPalette[color] || colorPalette['bg-brand-blue'];
    ctx.fillStyle = colors.wave;

    ctx.clearRect(0, 0, w, h); // Clear previous frame
    ctx.beginPath();
    ctx.moveTo(0, centerY);

    // Top Profile
    for (let i = 0; i < waveformData.length; i++) {
      const x = i * step;
      const amplitude = waveformData[i] * centerY * 0.7; 
      ctx.lineTo(x, centerY - amplitude);
    }

    // Bottom Profile
    for (let i = waveformData.length - 1; i >= 0; i--) {
      const x = i * step;
      const amplitude = waveformData[i] * centerY * 0.7;
      ctx.lineTo(x, centerY + amplitude);
    }

    ctx.closePath();
    ctx.fill();
  };

  onMount(() => {
    // 3. OBSERVE RESIZE (Triggers redraw when you Zoom)
    resizeObserver = new ResizeObserver(() => {
        // Wrap in requestAnimationFrame for performance
        window.requestAnimationFrame(draw);
    });

    if (container) {
        resizeObserver.observe(container);
    }

    // Cleanup on destroy
    return () => {
        resizeObserver?.disconnect();
    };
  });
</script>

<div 
  bind:this={container}
  class="relative h-[80%] rounded-lg border border-white/30 overflow-hidden flex items-center p-0.5 backdrop-blur-sm shadow-[inset_0_1px_4px_rgba(255,255,255,0.2)]"
  style="width: {width}%; background-color: {colorPalette[color]?.bg || 'rgba(191, 219, 254, 0.4)'};"
>
  
  <canvas 
    bind:this={canvas} 
    class="w-full h-full relative z-10"
    style="width: 100%; height: 100%;"
  ></canvas>

  <span class="absolute top-1 left-2 text-[9px] font-bold font-sans text-black/60 z-20 select-none uppercase tracking-wider mix-blend-multiply">
    Audio Clip
  </span>

</div>