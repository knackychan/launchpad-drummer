<script lang="ts">
  import { onMount } from 'svelte';
  import { ledToCss, padNote } from '../lib/led.ts';
  import type { PadInput, Rgb } from '../lib/led.ts';

  let { leds, names = [], selected = null, oninput, ondragpad, dropTarget = null, tag = '', invalid = false }: {
    ondragpad?: (note: number, event: PointerEvent) => void;
    dropTarget?: { x: number; y: number } | null;
    tag?: string;
    invalid?: boolean;
    leds: readonly Rgb[];
    /** Pad labels in frame order (top-left first); empty string = empty pad. */
    names?: readonly string[];
    selected?: number | null;
    oninput: (input: PadInput) => void;
  } = $props();
  let buttons: HTMLButtonElement[] = [];
  let focusIndex = $state(56);
  const held = new Set<number>();
  export function rects() { return buttons.filter(Boolean).map(button => { const note = Number(button.dataset.note); return { x: note % 10 - 1, y: Math.floor(note / 10) - 1, rect: button.getBoundingClientRect() }; }); }
  export function consume(note: number) { release(note); }

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
      class:empty={!names[index]}
      class:selected={selected === note}
      class:drop={dropTarget?.x === col - 1 && dropTarget?.y === row - 1 && !invalid}
      class:nodrop={dropTarget?.x === col - 1 && dropTarget?.y === row - 1 && invalid}
      style:--led={ledToCss(rgb)}
      data-note={note}
      aria-label="Row {row}, column {col}, {names[index] || 'empty'}"
      title={names[index] || 'Empty pad'}
      aria-pressed={selected === note}
      tabindex={focusIndex === index ? 0 : -1}
      onfocus={() => { focusIndex = index; }}
      onpointerdown={(event) => {
        if (event.button !== 0) return;
        event.currentTarget.setPointerCapture(event.pointerId);
        press(note);
        ondragpad?.(note, event);
      }}
      onpointerup={() => release(note)}
      onpointercancel={() => release(note)}
      onlostpointercapture={() => release(note)}
      onkeydown={(event) => {
        if (event.key === 'Enter') { event.preventDefault(); press(note); }
      }}
      onkeyup={(event) => {
        if (event.key === 'Enter') { event.preventDefault(); release(note); }
      }}
      onblur={() => release(note)}
    ><span class="glyph"></span>{#if names[index]}<span class="nm">{names[index]}</span>{/if}
      {#if dropTarget?.x === col - 1 && dropTarget?.y === row - 1 && !invalid}<span class="tag">{tag}</span>{/if}
    </button>
  {/each}
</div>
