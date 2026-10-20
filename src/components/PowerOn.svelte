<script lang="ts">
  import type { Startup } from '../lib/model.ts';
  let { startup, error, launchpad, lcxl, audio, busy = false, onnew, onopen, ondefault, onretry }: { startup: Startup; error: string | null; launchpad: string; lcxl: string; audio: string; busy?: boolean; onnew: () => void; onopen: () => void; ondefault: () => void; onretry?: () => void } = $props();
  // The last set may only be temporarily out of reach (a drive, a share): reading it again is a real option.
  const canRetry = $derived(!!onretry && (!!error || startup.kind === 'notFound' || startup.kind === 'unreadable'));
  const status = $derived(error ?? (startup.kind === 'notFound' ? `Last set not found: ${startup.path}` : startup.kind === 'unreadable' ? `Couldn't open ${startup.name} — set.json and backup are unreadable. Files left untouched.` : 'No set yet. Create one, open a set folder or start from the default set.'));
  const lamp = (s: string) => s === 'connected' || s === 'running' ? 'ok' : s === 'busy' ? 'warn' : ['lost', 'error', 'stopped', 'bootloader'].includes(s) ? 'error' : '';
</script>
<div class="power-screen"><div class="power plate">
  <h1 class="chromatic-fringe">LAUNCHPAD DRUMMER</h1><div class="sub" class:err={!!error || startup.kind === 'unreadable'} role="status">{status}</div>
  <div class="lamps">{#each [['LAUNCHPAD', launchpad], ['LCXL', lcxl], ['AUDIO', audio]] as [label, state]}<span class="silk"><i class="lamp {lamp(state)}"></i>{label}</span>{/each}</div>
  <div class="keys"><button class="key" disabled={busy} onclick={onnew}>NEW SET</button><button class="key" disabled={busy} onclick={onopen}>OPEN SET</button><button class="key" disabled={busy} onclick={ondefault}>DEFAULT SET</button>{#if canRetry}<button class="key" disabled={busy} onclick={onretry}>TRY AGAIN</button>{/if}</div>
</div></div>
<style>
  .power-screen { height: 100%; display: grid; place-items: center; }
  .power { width: 560px; max-width: 100%; padding: 28px 32px; display: flex; flex-direction: column; gap: 18px; align-items: center; text-align: center; }
  h1 { margin: 0; font: 700 30px/1.2 var(--f-lcd); letter-spacing: .18em; color: var(--lcd); }
  .chromatic-fringe { text-shadow: -1px 0 var(--orange), 1px 0 var(--vec); }
  .sub { font: 400 12px/1.5 var(--f-lcd); color: var(--lcd-dim); min-height: 18px; overflow-wrap: anywhere; }
  .sub.err { color: var(--error); }
  .lamps { display: flex; gap: 22px; } .lamps span { display: flex; align-items: center; gap: 6px; }
  .keys { display: flex; gap: 14px; } .keys .key { height: 44px; padding: 0 26px; font-size: 12px; }
</style>
