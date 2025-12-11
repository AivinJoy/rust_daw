<script lang="ts">
  import { onMount } from 'svelte';

  let { 
    color = 'bg-brand-blue', 
    waveform = null, 
    currentTime = 0,    
    startTime = 0,      
    duration = 0,
    zoom = 1 
  } = $props();

  let canvas: HTMLCanvasElement;
  let container: HTMLDivElement;
  let resizeObserver: ResizeObserver;

  const PIXELS_PER_SECOND = 50;

  const colorPalette: Record<string, { bg: string; wave: string; played: string }> = {
    'bg-brand-blue':  { bg: 'rgba(191, 219, 254, 0.1)', wave: '#60a5fa', played: '#2563eb' }, 
    'bg-brand-red':   { bg: 'rgba(254, 202, 202, 0.1)', wave: '#f87171', played: '#dc2626' }, 
    'bg-purple-500':  { bg: 'rgba(233, 213, 255, 0.1)', wave: '#c084fc', played: '#9333ea' },
    'bg-emerald-500': { bg: 'rgba(167, 243, 208, 0.1)', wave: '#34d399', played: '#059669' },
    'bg-orange-500':  { bg: 'rgba(254, 215, 170, 0.1)', wave: '#fb923c', played: '#ea580c' },
    'bg-pink-500':    { bg: 'rgba(251, 207, 232, 0.1)', wave: '#f472b6', played: '#db2777' }
  };

  const draw = () => {
    if (!canvas || !container) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = container.getBoundingClientRect();
    
    const pixelWidth = Math.ceil(rect.width * dpr);
    const pixelHeight = Math.ceil(rect.height * dpr);

    if (canvas.width !== pixelWidth || canvas.height !== pixelHeight) {
        canvas.width = pixelWidth;
        canvas.height = pixelHeight;
    }
    
    ctx.resetTransform();
    ctx.scale(dpr, dpr);

    const cssWidth = rect.width;
    const cssHeight = rect.height;
    const centerY = cssHeight / 2;

    const colors = colorPalette[color] || colorPalette['bg-brand-blue'];
    ctx.clearRect(0, 0, cssWidth, cssHeight);

    const timeElapsedInClip = currentTime - startTime;
    const progressX = timeElapsedInClip * PIXELS_PER_SECOND * zoom;

    if (waveform && waveform.mins && waveform.mins.length > 0) {
        const { mins, maxs } = waveform;
        const len = maxs.length;

        // --- FIX: Dynamic Step Size ---
        // Fits the data exactly to the container width
        const step = cssWidth / Math.max(1, len - 1);

        const defineWavePath = () => {
            ctx.beginPath();
            ctx.moveTo(0, centerY);

            // Top Half
            for (let i = 0; i < len - 1; i++) {
                const x = i * step;
                const y = centerY - (maxs[i] * centerY);
                
                const nextX = (i + 1) * step;
                const nextY = centerY - (maxs[i+1] * centerY);
                
                const midX = (x + nextX) / 2;
                const midY = (y + nextY) / 2;

                if (x > cssWidth) break; 

                if (i === 0) ctx.lineTo(x, y);
                ctx.quadraticCurveTo(x, y, midX, midY);
            }
            
            // Bottom Half
            let endIndex = len - 1;
            for (let i = endIndex; i > 0; i--) {
                const x = i * step;
                // Optimization: don't draw if far off screen right, but we need to close shape
                // For simplicity, just drawing all points is fine for now as we limited x above
                
                const y = centerY - (mins[i] * centerY); 
                const prevX = (i - 1) * step;
                const prevY = centerY - (mins[i-1] * centerY);
                const midX = (x + prevX) / 2;
                const midY = (y + prevY) / 2;

                ctx.quadraticCurveTo(x, y, midX, midY);
            }
            ctx.lineTo(0, centerY - (mins[0] * centerY));
            ctx.closePath();
        };

        ctx.save();
        defineWavePath();
        ctx.fillStyle = colors.wave;
        ctx.fill();
        ctx.restore();

        if (progressX > 0) {
            ctx.save();
            ctx.beginPath();
            ctx.rect(0, 0, progressX, cssHeight); 
            ctx.clip();
            defineWavePath();
            ctx.fillStyle = colors.played; 
            ctx.fill();
            ctx.restore();
        }

    } else {
        ctx.beginPath();
        ctx.moveTo(0, centerY);
        ctx.lineTo(cssWidth, centerY);
        ctx.strokeStyle = colors.wave;
        ctx.lineWidth = 2;
        ctx.stroke();
    }
  };

  $effect(() => {
     if (waveform && container) {
         requestAnimationFrame(draw);
     }
  });

  onMount(() => {
    resizeObserver = new ResizeObserver(() => window.requestAnimationFrame(draw));
    if (container) resizeObserver.observe(container);
    return () => resizeObserver?.disconnect();
  });
</script>

<div 
  bind:this={container}
  class="relative h-[85%] rounded-md overflow-hidden select-none ring-1 ring-white/10"
  style="background-color: {colorPalette[color]?.bg}; width: 100%;"
>
  <canvas bind:this={canvas} class="w-full h-full relative z-10 opacity-90"></canvas>
  <span class="absolute top-1 left-2 text-[9px] font-bold font-sans text-black/50 z-20 uppercase tracking-wider mix-blend-multiply">
    Audio Clip
  </span>
</div>