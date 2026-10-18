<script lang="ts">
  import { onMount } from 'svelte';
  import { ledToCss, padNote } from '../lib/led.ts';
  import type { PadInput, Rgb } from '../lib/led.ts';

  let { leds, selected = null, oninput }: {
    leds: readonly Rgb[]; selected?: number | null; oninput: (input: PadInput) => void;
  } = $props();
  let buttons: HTMLButtonElement[] = [];
  let focusIndex = $state(56);
  const held = new Set<number>();

  function press(note: number) {
    if (held.has(note)) return;
    held.add(note);
    oninput({ note, pressed: true, source: 'screen' });
  }
  function release(note: number) {
    if (!held.delete(note)) return;
    oninput({ note, pressed: false, source: 'screen' });
  }
  function releaseAll() { for (const note of held) release(note); }
  function navigate(event: KeyboardEvent, index: number) {
    const offsets: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -8, ArrowDown: 8 };
    if (event.key in offsets) {
      event.preventDefault();
      const col = index % 8;
      if ((event.key === 'ArrowLeft' && col === 0) || (event.key === 'ArrowRight' && col === 7)) return;
      focusIndex = Math.max(0, Math.min(63, index + offsets[event.key]));
      buttons[focusIndex]?.focus();
    }
  }
  onMount(() => {
    window.addEventListener('blur', releaseAll);
    return () => { window.removeEventListener('blur', releaseAll); releaseAll(); };
  });
</script>

<div class="pad-grid" role="group" aria-label="Launchpad 8 by 8 pads. Arrow keys move; Space or Enter presses a pad.">
  {#each leds as rgb, index}
    {@const row = 8 - Math.floor(index / 8)}
    {@const col = index % 8 + 1}
    {@const note = padNote(row, col)}
    <button
      bind:this={buttons[index]}
      class="pad"
      class:lit={rgb.some(v => v > 0)}
      class:selected={selected === note}
      style:--led={ledToCss(rgb)}
      data-note={note}
      aria-label="Row {row}, column {col}, note {note}"
      aria-pressed={selected === note}
      tabindex={focusIndex === index ? 0 : -1}
      onfocus={() => { focusIndex = index; }}
      onpointerdown={(event) => {
        if (event.button !== 0) return;
        event.currentTarget.setPointerCapture(event.pointerId);
        press(note);
      }}
      onpointerup={() => release(note)}
      onpointercancel={() => release(note)}
      onlostpointercapture={() => release(note)}
      onkeydown={(event) => {
        navigate(event, index);
        if (event.key === ' ' || event.key === 'Enter') { event.preventDefault(); press(note); }
      }}
      onkeyup={(event) => {
        if (event.key === ' ' || event.key === 'Enter') { event.preventDefault(); release(note); }
      }}
      onblur={() => release(note)}
    ></button>
  {/each}
</div>
