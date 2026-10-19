<script lang="ts">
  import Plate from './Plate.svelte';
  import RackUnit from './RackUnit.svelte';
  import type { CrtLevel } from '../lib/display.ts';
  let { level, onchange, status, events, reconnect }: { level: CrtLevel; onchange: (level: CrtLevel) => void; status: string; events: { time: string; text: string }[]; reconnect: () => void } = $props();
  const levels: CrtLevel[] = ['off', 'low', 'medium'];
</script>

<Plate label="MIDI DEVICES · AUTO RECONNECT" class="rack-unit midi-setup">
  <h1>MIDI DEVICES</h1>
  <div class="midi-state"><span class="lamp" class:connected={status === 'connected'} class:warn={status === 'busy'}></span><span class="lcd">LAUNCHPAD MK2 · {status.toUpperCase()}</span><button class="key" onclick={reconnect}>RETRY FRAME</button></div>
  <p class="lcd lcd-dim">The bridge scans every 2 seconds and restores the latest LED frame after reconnection.</p>
  <h2 class="silk">MIDI EVENT LOG</h2>
  <ol class="midi-log" aria-live="polite">
    {#if events.length === 0}<li class="lcd lcd-dim">Waiting for Launchpad status and pad events.</li>{/if}
    {#each events as event}<li><time class="lcd lcd-dim">{event.time}</time><span class="lcd">{event.text}</span></li>{/each}
  </ol>
</Plate>
<RackUnit title="DISPLAY">
  <fieldset>
    <legend class="silk">CRT EFFECTS</legend>
    <div class="switch">
      {#each levels as value}
        <label class:active={level === value}>
          <input type="radio" name="crt" {value} checked={level === value} onchange={() => onchange(value)} />
          {value.toUpperCase()}
        </label>
      {/each}
    </div>
  </fieldset>
  <p class="lcd lcd-dim">Glow, curved pixel mask, grain, vignette and an occasional scan line.</p>
  <p class="lcd lcd-dim">Reduced motion keeps the mask and grain still.</p>
</RackUnit>

<style>
  .midi-state { display:flex; align-items:center; gap:10px; }
  .midi-state .key { padding:5px 10px; }
  .lamp.connected { background:var(--ok); box-shadow:0 0 7px var(--ok); }
  .lamp.warn { background:var(--warn); }
  h2 { margin:22px 0 10px; }
  .midi-log { max-height:210px; overflow:auto; list-style:none; padding:0; margin:0; border:1px solid var(--line); background:var(--glass); }
  .midi-log li { display:grid; grid-template-columns:78px 1fr; gap:8px; padding:5px 8px; border-bottom:1px solid rgba(70,62,56,.5); font-size:11px; }
</style>
