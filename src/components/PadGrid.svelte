<script lang="ts">
  import { onMount } from 'svelte';
  import { ledToCss, padNote } from '../lib/led.ts';
  import type { PadInput, Rgb } from '../lib/led.ts';

  let { leds, names = [], labels = [], missing = [], selected = null, oninput, ondragpad, onmenu, dropTarget = null, tag = '', invalid = false }: {
    /** Right-click (or Shift+F10 / ContextMenu key): the pad and where to open the menu. */
    onmenu?: (x: number, y: number, at: { x: number; y: number }) => void;
    /** Screen-reader labels in frame order. */
    labels?: readonly string[];
    /** Frame-order flags: the pad's sample file is missing. */
    missing?: readonly boolean[];
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
  let grid: HTMLDivElement;
  const held = new Set<number>();
  let enterNote: number | null = null;
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
  const selectedXY = $derived(selected === null ? null : { x: (selected % 10) - 1, y: Math.floor(selected / 10) - 1 });
  /** Menu at the selected pad's rect (keyboard), or at the pointer for a pad that was right-clicked. */
  function openMenu(event: Event, x: number, y: number, at?: { x: number; y: number }) {
    event.preventDefault();
    if (at) { onmenu?.(x, y, at); return; }
    const button = buttons.find((b) => b && Number(b.dataset.note) === (y + 1) * 10 + x + 1);
    const r = button?.getBoundingClientRect();
    onmenu?.(x, y, r ? { x: r.left + r.width / 2, y: r.top + r.height / 2 } : { x: 0, y: 0 });
  }
  function gridKeydown(event: KeyboardEvent) {
    if (event.target !== grid || !selectedXY) return;
    if (event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey)) openMenu(event, selectedXY.x, selectedXY.y);
    else if (event.key === 'Enter') {
      event.preventDefault();
      if (!event.repeat && enterNote === null) { enterNote = selected; press(selected!); }
    }
  }
  function gridKeyup(event: KeyboardEvent) {
    if (event.target !== grid || event.key !== 'Enter') return;
    event.preventDefault();
    if (enterNote !== null) { release(enterNote); enterNote = null; }
  }
  // The keyboard opens the menu on keydown; a right-click in the gap between pads does nothing.
  function gridContextMenu(event: MouseEvent) {
    if (event.target === grid) event.preventDefault();
  }
  function releaseAll() { for (const note of held) release(note); enterNote = null; }
  onMount(() => {
    window.addEventListener('blur', releaseAll);
    return () => { window.removeEventListener('blur', releaseAll); releaseAll(); };
  });
</script>

<div
  bind:this={grid}
  class="pad-grid"
  role="grid"
  aria-label="Launchpad 8 by 8 pads. Arrow keys move; Space or Enter presses a pad."
  tabindex="0"
  aria-activedescendant={selectedXY ? `pad-${selectedXY.x}-${selectedXY.y}` : undefined}
  onkeydown={gridKeydown}
  onkeyup={gridKeyup}
  oncontextmenu={gridContextMenu}
  onblur={releaseAll}
>
  {#each { length: 8 } as _row, r}
    <div class="grow" role="row">
      {#each { length: 8 } as _col, c}
        {@const index = r * 8 + c}
        {@const rgb = leds[index]}
        {@const row = 8 - r}
        {@const col = c + 1}
        {@const note = padNote(row, col)}
        <button
          bind:this={buttons[index]}
          id="pad-{col - 1}-{row - 1}"
          class="pad"
          class:lit={rgb.some(v => v > 0)}
          class:empty={!names[index]}
          class:missing={missing[index]}
          class:selected={selected === note}
          class:drop={dropTarget?.x === col - 1 && dropTarget?.y === row - 1 && !invalid}
          class:nodrop={dropTarget?.x === col - 1 && dropTarget?.y === row - 1 && invalid}
          style:--led={ledToCss(rgb)}
          data-note={note}
          role="gridcell"
          aria-label={labels[index] || `Row ${row}, column ${col}, ${names[index] || 'empty'}`}
          aria-selected={selected === note}
          title={names[index] || 'Empty pad'}
          tabindex="-1"
          onmousedown={(event) => { event.preventDefault(); grid.focus({ preventScroll: true }); }}
          onpointerdown={(event) => {
            if (event.button !== 0) return;
            event.currentTarget.setPointerCapture(event.pointerId);
            press(note);
            ondragpad?.(note, event);
          }}
          onpointerup={() => release(note)}
          onpointercancel={() => release(note)}
          onlostpointercapture={() => release(note)}
          oncontextmenu={(event) => openMenu(event, col - 1, row - 1, { x: event.clientX, y: event.clientY })}
        ><span class="glyph"></span>{#if names[index]}<span class="nm">{names[index]}</span>{/if}{#if missing[index]}<span class="miss" aria-hidden="true">!</span>{/if}
          {#if dropTarget?.x === col - 1 && dropTarget?.y === row - 1 && !invalid}<span class="tag">{tag}</span>{/if}
        </button>
      {/each}
    </div>
  {/each}
</div>

<style>
  .grow { display: contents; }
  .miss { position: absolute; top: 3px; right: 5px; color: #fff; font: 700 calc(var(--pad) * .24)/1 var(--f-silk); text-shadow: 0 1px 2px #000, 0 0 4px #000; pointer-events: none; }
  /* Reduced motion: the hardware blink becomes a steady red pad on screen; the ! glyph stays. */
  @media (prefers-reduced-motion: reduce) {
    .pad.missing {
      background: radial-gradient(circle at 50% 45%, #ff9a92 0, #ff3b30 55%, #b3261e 100%);
      box-shadow: inset 0 1px 0 rgba(255,255,255,.5), 0 4px 0 #7a1812, 0 7px 0 rgba(0,0,0,.3), 0 11px 16px rgba(0,0,0,.6), 0 0 calc(var(--pad) * .28) rgba(255,59,48,.85);
    }
  }
</style>
