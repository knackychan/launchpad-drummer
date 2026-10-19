<script lang="ts">
  import RackUnit from './RackUnit.svelte';
  import { fixedRole, NO_DEVICES, statusLamp, STATUS_TEXT } from '../lib/midi.ts';
  import type { MidiDevice } from '../lib/model.ts';
  let { devices, events, onrole }: {
    devices: MidiDevice[]; events: { time: string; text: string }[];
    onrole: (port: string, role: 'keys' | 'other') => void;
  } = $props();
</script>

<RackUnit title="MIDI DEVICES">
  {#if devices.length === 0}
    <div class="screen empty"><span class="lcd lcd-dim">{NO_DEVICES}</span></div>
  {:else}
    <table class="tbl screen">
      <thead><tr><th></th><th>Port</th><th>Role</th><th>Status</th></tr></thead>
      <tbody>
        {#each devices as device (device.port)}
          {@const lamp = statusLamp(device.status)}
          <tr>
            <td class="lamp-cell"><span class="lamp {lamp}" role="img" aria-label={STATUS_TEXT[device.status]}></span></td>
            <td class="lcd">{device.port}</td>
            <td>
              {#if fixedRole(device.role)}
                <span class="silk">{fixedRole(device.role)}</span>
              {:else}
                <div class="switch" role="radiogroup" aria-label="Role of {device.port}">
                  <button class="key small" class:on={device.role === 'keys'} role="radio" aria-checked={device.role === 'keys'} onclick={() => onrole(device.port, 'keys')}>KEYS</button>
                  <button class="key small" class:on={device.role === 'other'} role="radio" aria-checked={device.role === 'other'} onclick={() => onrole(device.port, 'other')}>OTHER</button>
                </div>
              {/if}
            </td>
            <td class="lcd status" class:warn={lamp === 'warn'} class:error={lamp === 'error'}>{STATUS_TEXT[device.status]}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
  <h2 class="silk">MIDI EVENT LOG</h2>
  <ol class="midi-log" aria-live="polite">
    {#if events.length === 0}<li class="lcd lcd-dim">Waiting for Launchpad status and pad events.</li>{/if}
    {#each events as event}<li><time class="lcd lcd-dim">{event.time}</time><span class="lcd">{event.text}</span></li>{/each}
  </ol>
</RackUnit>

<style>
  .empty { padding: 12px; }
  .tbl { width: 100%; border-collapse: collapse; font-size: 12px; }
  .tbl th, .tbl td { padding: 7px 10px; text-align: left; border-bottom: 1px solid rgba(255,255,255,.05); }
  .tbl th { font: 600 10px var(--f-silk); letter-spacing: .08em; color: var(--silk); text-transform: uppercase; }
  .lamp-cell { width: 24px; }
  .status { color: var(--lcd-dim); }
  .status.warn { color: var(--warn); }
  .status.error { color: var(--error); }
  h2 { margin: 22px 0 10px; }
  .midi-log { max-height: 210px; overflow: auto; list-style: none; padding: 0; margin: 0; border: 1px solid var(--line); background: var(--glass); }
  .midi-log li { display: grid; grid-template-columns: 78px 1fr; gap: 8px; padding: 5px 8px; border-bottom: 1px solid rgba(70,62,56,.5); font-size: 11px; }
</style>
