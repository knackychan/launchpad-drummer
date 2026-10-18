// Per-channel activity from pad hits (0..1, ~110 ms half-life). Feeds the top-strip chips, the mixer
// level fills and the telemetry orbit radar. Real audio levels need engine meter events (not streamed yet).
import { dbToPos } from './mix.ts';

export const energy = new Float32Array(8);

export function hitGroup(group: number, amount: number): void {
  if (group >= 0 && group < 8) energy[group] = Math.max(energy[group], Math.min(1, Math.max(0, amount)));
}

/** Starts the decay loop; it also drives `[data-chip]` opacity and `.f-fill[data-group]` heights. */
export function runActivity(): () => void {
  let raf = 0, last = performance.now();
  const frame = (now: number) => {
    const k = Math.pow(0.5, Math.min(100, now - last) / 110);
    last = now;
    for (let i = 0; i < 8; i++) energy[i] *= k;
    document.querySelectorAll<HTMLElement>('[data-chip]').forEach((el) => {
      el.style.opacity = (0.16 + 0.84 * energy[Number(el.dataset.chip)]).toFixed(2);
    });
    document.querySelectorAll<HTMLElement>('.f-fill[data-group]').forEach((el) => {
      el.style.setProperty('--m', (energy[Number(el.dataset.group)] * 0.75).toFixed(3));
    });
    raf = requestAnimationFrame(frame);
  };
  raf = requestAnimationFrame(frame);
  return () => cancelAnimationFrame(raf);
}

/** Activity of a pad hit: its channel fader position, nothing when the channel is silenced. */
export function hitAmount(volumeDb: number, audible: boolean): number {
  return audible ? Math.min(1, dbToPos(volumeDb) / 0.75) : 0;
}
