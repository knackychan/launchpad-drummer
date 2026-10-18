<script lang="ts">
  import { onMount } from 'svelte';
  import Plate from './components/Plate.svelte';
  import HazardCap from './components/HazardCap.svelte';
  import PadGrid from './components/PadGrid.svelte';
  import CrtLayers from './components/CrtLayers.svelte';
  import SetupDisplay from './components/SetupDisplay.svelte';
  import HordeGame from './components/HordeGame.svelte';
  import ConsoleBackdrop from './components/ConsoleBackdrop.svelte';
  import { applyLedUpdates, createLedFrame } from './lib/led.ts';
  import type { PadInput } from './lib/led.ts';
  import { CRT_FACTORS, readDisplay, saveDisplay } from './lib/display.ts';
  import type { CrtLevel } from './lib/display.ts';

  let mode = $state<'play' | 'games' | 'setup'>('play');
  let game = $state('horde');
  let deviceState = $state('disconnected');
  let midiLog = $state<{ time: string; text: string }[]>([]);
  let crt = $state<CrtLevel>('low');
  let leds = $state(createLedFrame());
  let selected = $state<number | null>(null);
  function logMidi(text: string) {
    midiLog = [{ time: new Date().toLocaleTimeString(), text }, ...midiLog].slice(0, 40);
  }
  function sendFrame(frame: readonly (readonly [number, number, number])[]) {
    leds = frame.map(([r, g, b]) => [r, g, b] as const);
    try { void window.__TAURI__?.core.invoke('send_led_frame', { frame }).catch(() => {}); } catch { /* Browser preview has no MIDI bridge. */ }
  }

  onMount(() => {
    try { crt = readDisplay(window.localStorage); } catch { crt = 'low'; }
    const api = window.__TAURI__?.event;
    if (!api) return;
    let unlistenStatus = () => {}, unlistenPad = () => {};
    void api.listen<{ state: string; message: string }>('launchpad-status', event => {
      deviceState = event.payload.state; logMidi(event.payload.message);
    }).then(fn => unlistenStatus = fn);
    void api.listen<{ note: number; pressed: boolean }>('launchpad-pad', event => {
      logMidi(`PAD ${event.payload.note} ${event.payload.pressed ? 'DOWN' : 'UP'}`);
      padInput({ ...event.payload, source: 'hardware' });
    }).then(fn => unlistenPad = fn);
    return () => { unlistenStatus(); unlistenPad(); };
  });
  let hordeComponent = $state<{ padInput: (note: number, pressed: boolean) => void } | undefined>();
  function setCrt(level: CrtLevel) {
    crt = level;
    try { saveDisplay(window.localStorage, level); } catch { /* The switch still works without storage. */ }
  }
  function padInput(input: PadInput) {
    if (mode === 'games' && game === 'horde') { hordeComponent?.padInput(input.note, input.pressed); return; }
    if (input.note % 10 === 9) return;
    if (input.pressed) selected = input.note;
    sendFrame(applyLedUpdates(leds, [{ note: input.note, rgb: input.pressed ? [63, 63, 63] : [0, 0, 0] }]));
  }
</script>

<div class="instrument" data-crt={crt} style:--crt={CRT_FACTORS[crt]}>
  <div class="console">
    <ConsoleBackdrop />
    <header class="top-strip">
      <div class="brand" aria-label="Launchpad Drummer">LPD·01</div>
      <button class="set-lcd screen" disabled title="Set management — not fitted yet">
        <span class="silk">SET</span><span class="lcd">NO SET</span>
      </button>
      <button class="key small" aria-label="Undo" disabled>
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M7 4 3 8l4 4M3 8h8a5 5 0 0 1 0 10" /></svg>
      </button>
      <button class="key small" aria-label="Redo" disabled>
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m13 4 4 4-4 4m4-4H9a5 5 0 0 0 0 10" /></svg>
      </button>
      <div class="notice screen lcd" role="status">{deviceState === 'connected' ? 'Launchpad MK2 connected' : deviceState === 'busy' ? 'Launchpad port busy — close other MIDI apps' : deviceState === 'bootloader' ? 'Launchpad bootloader — run updater' : 'Launchpad not connected — on-screen pads available'}</div>
    </header>

    <nav class="mode-rail" aria-label="Mode">
      <button class="key mode-key" class:on={mode === 'play'} aria-current={mode === 'play' ? 'page' : undefined} onclick={() => { mode = 'play'; }}>PLAY</button>
      <button class="key mode-key blank" aria-disabled="true" aria-label="TRAIN — not fitted yet" title="TRAIN — not fitted yet"></button>
      <button class="key mode-key" class:on={mode === 'games'} aria-current={mode === 'games' ? 'page' : undefined} onclick={() => { mode = 'games'; }}>GAMES</button>
      {#each ['BAND'] as name}
        <button class="key mode-key blank" aria-disabled="true" aria-label="{name} — not fitted yet" title="{name} — not fitted yet"></button>
      {/each}
      <div class="rail-spacer"></div>
      <button class="key mode-key" class:on={mode === 'setup'} aria-current={mode === 'setup' ? 'page' : undefined} onclick={() => { mode = 'setup'; }}>SETUP</button>
    </nav>

    <main class="bay" aria-label={mode === 'play' ? 'Play' : mode === 'games' ? 'Games' : 'Setup'}>
      {#if mode === 'play'}
        <div class="play-stage">
          <div class="pair">
            <Plate label="GRID · LAUNCHPAD MK2" class="launchpad">
              <div class="top-buttons" aria-hidden="true">
                {#each Array(8) as _}<span class="round-cap"></span>{/each}
              </div>
              <div class="grid-with-side">
              <PadGrid {leds} {selected} oninput={padInput} />
                <div class="side-buttons" aria-hidden="true">
                  {#each Array(8) as _, index}
                    <div class="bank"><span class="round-cap" class:bank-active={index === 0}></span><span class="screen lcd" class:lcd-dim={index !== 0}>BANK {index + 1}</span></div>
                  {/each}
                </div>
              </div>
              <div class="grid-footer silk">GRID MIRROR <span class="lcd lcd-dim">{selected === null ? '—' : `R${Math.floor(selected / 10)} · C${selected % 10}`}</span></div>
            </Plate>
            <Plate label="MIX · LAUNCH CONTROL XL" class="mixer-cap"><HazardCap label="Mixer" /></Plate>
          </div>
          <Plate label="KEYS · KEYSTATION 49" class="keys-cap"><HazardCap label="Keyboard" /></Plate>
          <Plate label="SEQUENCER · STEP GRID" class="sequencer-cap"><HazardCap label="Sequencer" /></Plate>
        </div>
      {:else if mode === 'games'}
        <div class="games-stage">
          <aside class="game-picker plate" aria-label="Game picker" data-label="GAMES">
            <h1 class="silk">SELECT GAME</h1>
            {#each [{ id: 'horde', title: 'HORDE', desc: 'HOLD THE BEACH · PLAYABLE' }, { id: 'chess', title: 'CHESS', desc: 'PROTOTYPE' }, { id: 'xiangqi', title: 'XIANGQI', desc: 'PROTOTYPE' }, { id: 'cockpit', title: 'COCKPIT', desc: 'PROTOTYPE' }] as entry}
              <button class="game-choice key" class:on={game === entry.id} onclick={() => { game = entry.id; }}><span>{entry.title}</span><small class="lcd lcd-dim">{entry.desc}</small></button>
            {/each}
          </aside>
          <Plate label={game === 'horde' ? 'HORDE · HOLD THE BEACH' : `${game.toUpperCase()} · PROTOTYPE`} class="game-plate">
            {#if game === 'horde'}
              <HordeGame bind:this={hordeComponent} onframe={sendFrame} />
            {:else}
              <iframe class="prototype" title={`${game} prototype`} src={`/games/${game}/index.html`} sandbox="allow-scripts"></iframe>
            {/if}
          </Plate>
          <div class="game-mirror plate" data-label="LIVE LAUNCHPAD MIRROR">
            <PadGrid {leds} {selected} oninput={padInput} />
          </div>
        </div>
      {:else}
        <SetupDisplay level={crt} status={deviceState} events={midiLog} reconnect={() => sendFrame(leds)} onchange={setCrt} />
      {/if}
    </main>

    <footer class="status-strip">
      {#each ['LAUNCHPAD', 'LCXL', 'AUDIO'] as device}
        <button class="device-status" onclick={() => { mode = 'setup'; }} aria-label="{device}: {device === 'LAUNCHPAD' ? deviceState : device === 'AUDIO' ? 'stopped' : 'not connected'}. Open Setup">
          <span class="lamp" class:stopped={device === 'AUDIO'} class:connected={device === 'LAUNCHPAD' && deviceState === 'connected'} class:warn={device === 'LAUNCHPAD' && deviceState === 'busy'} class:bootloader={device === 'LAUNCHPAD' && deviceState === 'bootloader'}></span><span class="silk">{device}</span>
        </button>
      {/each}
      <span class="lcd lcd-dim">AUDIO STOPPED</span>
      <span class="status-spacer"></span>
      <span class="lcd lcd-dim">NO SET</span><span class="lamp" title="No pending save"></span>
    </footer>
  </div>
  <CrtLayers level={crt} />
</div>
