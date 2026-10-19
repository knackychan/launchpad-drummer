<script lang="ts">
  import { onMount } from 'svelte';
  import RackUnit from './RackUnit.svelte';
  import { invoke } from '../lib/api.ts';
  import { deviceOptions, latencyText, streamView } from '../lib/audio.ts';
  import type { AppConfig, AsioInfo, AudioDevice, AudioPrefs, AudioStatus } from '../lib/model.ts';
  let { status, dropoutRecent, onstatus, onerror }: {
    status: AudioStatus | null; dropoutRecent: boolean;
    onstatus: (status: AudioStatus) => void; onerror: (text: string) => void;
  } = $props();

  let prefs = $state<AudioPrefs>({ backend: 'wasapi', device: null, bufferFrames: null });
  let devices = $state<AudioDevice[]>([]);
  let asio = $state<AsioInfo>({ available: false, reason: null });
  let busy = $state(false);

  const asioOn = $derived(prefs.backend === 'asio');
  const device = $derived(deviceOptions(devices, prefs.device));
  const buffers = $derived(devices.find(d => d.name === device.selected)?.bufferSizes ?? []);
  const buffer = $derived(prefs.bufferFrames !== null && buffers.includes(prefs.bufferFrames) ? String(prefs.bufferFrames) : '');
  const stream = $derived(streamView(status, dropoutRecent));
  const errorText = (e: unknown) => String(e instanceof Error ? e.message : e);

  async function loadDevices(backend: string) {
    try { devices = (await invoke<AudioDevice[]>('audio_devices', { backend })) ?? []; }
    catch (e) { devices = []; onerror(errorText(e)); }
  }
  async function loadPrefs() {
    prefs = (await invoke<AppConfig>('get_config')).audio;
    await loadDevices(prefs.backend);
  }
  onMount(() => {
    void loadPrefs().catch(e => onerror(errorText(e)));
    void invoke<AsioInfo>('asio_available').then(info => { if (info) asio = info; }).catch(e => onerror(errorText(e)));
  });

  /** Save the prefs and restart the stream; the returned status replaces the app's. The UI stays live meanwhile. */
  async function apply(next: AudioPrefs) {
    if (busy) return;
    busy = true;
    try {
      if (next.backend !== prefs.backend) await loadDevices(next.backend);
      prefs = next;
      const result = await invoke<AudioStatus>('set_audio', { prefs: next });
      if (result) onstatus(result);
    } catch (e) {
      onerror(`Audio not restarted — ${errorText(e)}`);
      await refresh();
    } finally { busy = false; }
  }
  /** After a failed restart: what the backend actually has now. */
  async function refresh() {
    try {
      const result = await invoke<AudioStatus>('get_audio_status');
      if (result) onstatus(result);
    } catch (e) { onerror(errorText(e)); }
  }
  function chooseBackend(backend: 'asio' | 'wasapi') {
    if (backend === prefs.backend || (backend === 'asio' && !asio.available)) return;
    void apply({ backend, device: null, bufferFrames: null });
  }
  function chooseDevice(name: string) {
    const sizes = devices.find(d => d.name === name)?.bufferSizes ?? [];
    void apply({ ...prefs, device: name, bufferFrames: prefs.bufferFrames !== null && sizes.includes(prefs.bufferFrames) ? prefs.bufferFrames : null });
  }
  function chooseBuffer(value: string) { void apply({ ...prefs, bufferFrames: value === '' ? null : Number(value) }); }
  async function resetDropouts() {
    try { await invoke('reset_dropouts'); await refresh(); } catch (e) { onerror(errorText(e)); }
  }
  async function reconnect() {
    if (!stream.canReconnect || busy) return;
    busy = true;
    try { const result = await invoke<AudioStatus>('audio_reconnect'); if (result) onstatus(result); }
    catch (e) { onerror(`Audio not reconnected — ${errorText(e)}`); await refresh(); }
    finally { busy = false; }
  }
</script>

<RackUnit title="AUDIO">
  <div class="rows" aria-busy={busy}>
    <span class="silk">BACKEND</span>
    <div class="switch" role="radiogroup" aria-label="Audio backend">
      <button class="key" class:on={asioOn} role="radio" aria-checked={asioOn} aria-disabled={!asio.available}
        title={asio.available ? undefined : asio.reason ?? undefined} onclick={() => chooseBackend('asio')}>ASIO</button>
      <button class="key" class:on={!asioOn} role="radio" aria-checked={!asioOn} onclick={() => chooseBackend('wasapi')}>WASAPI</button>
    </div>

    <span class="silk">DEVICE</span>
    <select class="lcd screen" aria-label="Audio device" disabled={busy || device.options.length === 0} value={device.selected} onchange={e => chooseDevice(e.currentTarget.value)}>
      {#each device.options as option}<option value={option.value}>{option.label}</option>{/each}
    </select>

    <span class="silk">BUFFER</span>
    <select class="lcd screen" aria-label="Buffer size" disabled={busy} value={buffer} onchange={e => chooseBuffer(e.currentTarget.value)}>
      <option value="">AUTO</option>
      {#each buffers as frames}<option value={String(frames)}>{frames} samples</option>{/each}
    </select>

    <span class="silk">LATENCY</span>
    <span class="screen readout lcd">{status ? latencyText(status.latency) : 'MIDI→BUFFER – · OUTPUT – · TOTAL – ms'}</span>

    <span class="silk">DROPOUTS</span>
    <div class="line">
      <span class="screen readout lcd">{status?.dropouts ?? 0}</span>
      <button class="key small" onclick={() => void resetDropouts()}>RESET</button>
      {#if (status?.dropouts ?? 0) > 0}<span class="lcd lcd-dim hint">Raise buffer size</span>{/if}
    </div>

    <span class="silk">STREAM</span>
    <div class="line">
      <span class="lamp {stream.lamp}" role="img" aria-label={stream.text}></span>
      <span class="lcd state">{stream.text}</span>
      <button class="key small" class:on={stream.canReconnect} aria-disabled={!stream.canReconnect || busy} onclick={() => void reconnect()}>RECONNECT</button>
    </div>
  </div>
</RackUnit>

<style>
  .rows { display: grid; grid-template-columns: 120px 1fr; gap: 10px 16px; align-items: center; }
  .rows > .switch { justify-self: start; }
  .line { display: flex; align-items: center; gap: 10px; }
  select.lcd { appearance: none; height: 28px; padding: 0 28px 0 10px; font-size: 12px; border: 0; cursor: pointer; justify-self: start; min-width: 260px; max-width: 100%;
    background-image: linear-gradient(45deg, transparent 50%, var(--lcd-dim) 50%), linear-gradient(135deg, var(--lcd-dim) 50%, transparent 50%);
    background-position: calc(100% - 14px) 12px, calc(100% - 9px) 12px; background-size: 5px 5px; background-repeat: no-repeat; }
  select.lcd:disabled { cursor: default; opacity: .6; }
  select.lcd option { background: var(--ink); color: var(--lcd); }
  .readout { display: inline-flex; align-items: center; gap: 10px; height: 28px; padding: 0 10px; font-size: 12px; justify-self: start; }
  .state { font-size: 12px; }
  .hint { font-size: 11px; }
</style>
