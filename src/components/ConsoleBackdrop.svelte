<script lang="ts">
  import { onMount } from 'svelte';
  let canvas: HTMLCanvasElement;
  onMount(() => {
    const base = document.createElement('canvas');
    const motion = matchMedia('(prefers-reduced-motion: reduce)');
    const colours = ['255,80,60', '92,255,157', '255,174,43', '79,208,198', '65,140,255', '255,79,216', '255,246,220'];
    let ctx: CanvasRenderingContext2D | null;
    let timer: number;
    let w = 0, h = 0;
    let sparks: { x: number; y: number; born: number; life: number; colour: string; run: number }[] = [];
    let lastSpawn = 0;
    const noise = (x: number, y: number) => {
      const value = Math.sin(x * 127.1 + y * 311.7) * 43758.5453;
      return value - Math.floor(value);
    };
    function resize() {
      const rect = canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      w = rect.width; h = rect.height;
      canvas.width = base.width = Math.round(w * dpr);
      canvas.height = base.height = Math.round(h * dpr);
      ctx = canvas.getContext('2d');
      const b = base.getContext('2d');
      if (!ctx || !b) return;
      b.scale(dpr, dpr);
      for (let y = 7; y < h; y += 14) for (let x = 7; x < w; x += 14) {
        const n = noise(x, y);
        const colour = colours[Math.floor(noise(Math.floor(x / 180), Math.floor(y / 180)) * colours.length)];
        b.fillStyle = '#110f0e'; b.beginPath(); b.arc(x, y, 2.2, 0, Math.PI * 2); b.fill();
        b.fillStyle = `rgba(${colour},${n < .3 ? .03 : .13 + n * .22})`;
        b.beginPath(); b.arc(x, y, .8, 0, Math.PI * 2); b.fill();
        if (n > .985) {
          b.strokeStyle = `rgba(${colour},.2)`; b.strokeRect(x - 4, y - 4, 8, 8);
          b.fillRect(x - 1.5, y - 1.5, 3, 3);
        }
      }
      b.strokeStyle = 'rgba(79,208,198,.2)'; b.lineWidth = 1;
      for (let i = 0; i < 9; i++) {
        const x = Math.floor(noise(i, 5) * w / 14) * 14;
        const y = Math.floor(noise(i, 9) * h / 14) * 14;
        b.beginPath(); b.moveTo(x, y); b.lineTo(x + 70, y); b.lineTo(x + 98, y + 28); b.lineTo(x + 168, y + 28); b.stroke();
        b.beginPath(); b.arc(x, y, 2, 0, Math.PI * 2); b.stroke();
      }
      sparks = [];
      paint();
    }
    function paint() {
      if (!ctx) return;
      const time = performance.now();
      const dpr = window.devicePixelRatio || 1;
      ctx.clearRect(0, 0, canvas.width, canvas.height); ctx.drawImage(base, 0, 0);
      if (motion.matches) return;
      if (time - lastSpawn > 83) {
        for (let i = 0; i < 3; i++) sparks.push({ x: 7 + Math.floor(Math.random() * w / 14) * 14, y: 7 + Math.floor(Math.random() * h / 14) * 14, born: time, life: 1800 + Math.random() * 4200, colour: colours[Math.floor(Math.random() * colours.length)], run: Math.random() < .3 ? 3 + Math.floor(Math.random() * 6) : 1 });
        lastSpawn = time;
      }
      sparks = sparks.filter(s => time - s.born < s.life);
      ctx.save(); ctx.scale(dpr, dpr);
      for (const spark of sparks) {
        const alpha = .6 * (1 - (time - spark.born) / spark.life);
        ctx.fillStyle = `rgba(${spark.colour},${alpha})`; ctx.shadowColor = `rgb(${spark.colour})`; ctx.shadowBlur = 4;
        for (let i = 0; i < spark.run; i++) { ctx.beginPath(); ctx.arc(spark.x + i * 14, spark.y, 1, 0, Math.PI * 2); ctx.fill(); }
      }
      ctx.restore();
    }
    function setMotion() {
      clearInterval(timer); paint();
      if (!motion.matches) timer = window.setInterval(paint, 1000 / 12);
    }
    const observer = new ResizeObserver(resize);
    observer.observe(canvas); motion.addEventListener('change', setMotion);
    resize(); setMotion();
    return () => { clearInterval(timer); observer.disconnect(); motion.removeEventListener('change', setMotion); };
  });
</script>

<canvas bind:this={canvas} class="console-backdrop" aria-hidden="true"></canvas>

<style>
  .console-backdrop { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; }
</style>
