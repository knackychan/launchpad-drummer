<script lang="ts">
  import type { LoadStatus } from '../lib/model.ts';
  import { announcement, isSlow, isTerminal, phaseLabel, progressOf } from '../lib/startup.ts';
  let { status, error = null, launchpad, lcxl, audio, overlay = false, onretry, onopen, onreload }: {
    status: LoadStatus; error?: string | null; launchpad: string; lcxl: string; audio: string; overlay?: boolean;
    onretry?: () => void; onopen?: () => void; onreload?: () => void;
  } = $props();
  const lamp = (s: string) => s === 'connected' || s === 'running' ? 'ok' : s === 'busy' || s === 'opening' ? 'warn' : ['lost', 'error', 'stopped', 'bootloader'].includes(s) ? 'error' : '';

  // "Still loading" after ten seconds of the same request: feedback only, nothing is cancelled.
  let since = $state(performance.now());
  let now = $state(performance.now());
  const request = $derived(status.requestId); // changes only with a new request, not with each progress update
  $effect(() => { request; since = now = performance.now(); });
  $effect(() => {
    const timer = window.setInterval(() => { now = performance.now(); }, 1000);
    return () => window.clearInterval(timer);
  });
  const progress = $derived(progressOf(status));
  const working = $derived(!isTerminal(status.phase) && !error);
  const slow = $derived(working && isSlow(now - since));
  const label = $derived(error ? 'Startup stopped' : phaseLabel(status));
</script>
<div class="startup" class:overlay><div class="card plate">
  <h1 class="chromatic-fringe">LAUNCHPAD DRUMMER</h1>
  <div class="phase" aria-hidden="true">{label}</div>
  <span class="sr" role="status">{error ? '' : announcement(status)}</span>
  {#if working}
    {#if progress.kind === 'determinate'}
      <progress value={progress.value} max={progress.max} aria-label="Loading samples"></progress>
    {:else}
      <progress aria-label="Working"></progress>
    {/if}
  {/if}
  {#if status.failed > 0 && !error}<div class="note warn">{status.failed} sample{status.failed === 1 ? '' : 's'} could not be loaded so far.</div>{/if}
  {#if error}<div class="note err" role="alert">{error}</div>{/if}
  {#if slow}<div class="note">Still loading — this can take a while for a large set. The window stays usable.</div>{/if}
  <div class="lamps">{#each [['LAUNCHPAD', launchpad], ['LCXL', lcxl], ['AUDIO', audio]] as [name, state]}<span class="silk"><i class="lamp {lamp(state)}"></i>{name}{name === 'AUDIO' && state === 'opening' ? ' · OPENING' : ''}</span>{/each}</div>
  {#if error || slow}
    <div class="keys">
      {#if error && onretry}<button class="key" onclick={onretry}>TRY AGAIN</button>{/if}
      {#if onopen}<button class="key" onclick={onopen}>OPEN ANOTHER SET</button>{/if}
      {#if error && onreload}<button class="key" onclick={onreload}>RELOAD</button>{/if}
    </div>
  {/if}
</div></div>
<style>
  .startup { height: 100%; display: grid; place-items: center; }
  .startup.overlay { position: absolute; inset: 0; z-index: 20; background: rgba(0, 0, 0, .6); }
  .card { width: 560px; max-width: 100%; padding: 28px 32px; display: flex; flex-direction: column; gap: 16px; align-items: center; text-align: center; }
  h1 { margin: 0; font: 700 30px/1.2 var(--f-lcd); letter-spacing: .18em; color: var(--lcd); }
  .chromatic-fringe { text-shadow: -1px 0 var(--orange), 1px 0 var(--vec); }
  .phase { font: 400 14px/1.5 var(--f-lcd); color: var(--lcd); min-height: 21px; overflow-wrap: anywhere; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  progress { width: 100%; max-width: 380px; height: 10px; accent-color: var(--orange); }
  .note { font: 400 12px/1.5 var(--f-lcd); color: var(--lcd-dim); overflow-wrap: anywhere; }
  .note.err { color: var(--error); }
  .note.warn { color: var(--warn); }
  .lamps { display: flex; gap: 22px; } .lamps span { display: flex; align-items: center; gap: 6px; }
  .keys { display: flex; gap: 14px; flex-wrap: wrap; justify-content: center; } .keys .key { height: 44px; padding: 0 22px; font-size: 12px; }
  @media (prefers-reduced-motion: reduce) {
    progress:indeterminate::-webkit-progress-value { animation: none; opacity: .5; }
    progress:indeterminate { animation: none; }
  }
</style>
