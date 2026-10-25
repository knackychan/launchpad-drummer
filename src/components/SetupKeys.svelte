<script lang="ts">
  import RackUnit from './RackUnit.svelte';
  import { middleTruncate } from '../lib/format.ts';
  import type { KeysStatus } from '../lib/model.ts';

  let { status, soundfont, onload, onclear, onengine, patches, onshowpatches }: {
    status: KeysStatus;
    /** The `.sf2` path in the app config, if any. */
    soundfont: string | null;
    onload: () => void;
    onclear: () => void;
    onengine: (engine: 'synth' | 'soundfont') => void; patches: string; onshowpatches: () => void;
  } = $props();
</script>

<RackUnit title="KEYS">
  <div class="rows">
    <div class="row"><span class="silk">ENGINE</span><div class="switch"><button class="key small" class:on={status.engine==='synth'} aria-pressed={status.engine==='synth'} onclick={()=>onengine('synth')}>SYNTH</button><button class="key small" class:on={status.engine==='soundfont'} aria-pressed={status.engine==='soundfont'} aria-disabled={!soundfont} title={!soundfont?'Load a SoundFont first':status.name} onclick={()=>{if(soundfont)onengine('soundfont');}}>SOUNDFONT</button></div></div>
    <div class="row">
      <span class="silk">SOUNDFONT</span>
      <span class="lcd screen val" title={soundfont ?? ''}>{soundfont ? middleTruncate(soundfont, 40) : 'NONE'}</span>
      <button type="button" class="key" onclick={onload}>LOAD…</button>
      <button type="button" class="key" aria-disabled={!soundfont} onclick={() => { if (soundfont) onclear(); }}>CLEAR</button>
    </div>
    {#if status.error}<p class="error" role="alert">{status.error}</p>{/if}
    <div class="row"><span class="silk">PATCHES</span><span class="lcd screen val" title={patches}>{middleTruncate(patches,40)}</span><button class="key small" onclick={onshowpatches}>SHOW</button></div>
    <p class="hint">The synth plays the keys; a General MIDI SoundFont (.sf2) can replace it.</p>
  </div>
</RackUnit>

<style>
  .rows { display: flex; flex-direction: column; gap: 10px; }
  .row { display: flex; align-items: center; gap: 10px; }
  .row .silk { width: 92px; flex: none; }
  .val { flex: 1; min-width: 0; padding: 6px 10px; font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .error { margin: 0; color: var(--error); font: 400 12px/1.4 var(--f-lcd); }
  .hint { margin: 0; color: var(--lcd-dim); font: 400 11px/1.5 var(--f-lcd); }
</style>
