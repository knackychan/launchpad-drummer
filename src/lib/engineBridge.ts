// Dustline LIVE SYNTH bridge (ship engine spec 5.2). The game frame asks for a native synth voice with
// `{lpd: 'engine-synth', op: 'start' | 'frame' | 'stop'}` messages; this module validates them, caps their rate and
// drives the app's game engine voice through an injected host, so it is tested without Tauri or a frame.

export const ENGINE_PATCH_MAX = 16384;
/** Messages per second, and how many may arrive at once (frames come at 40 Hz). */
export const ENGINE_RATE = 60;
export const ENGINE_BURST = 10;

export type EngineMessage =
  | { op: 'start'; patch: string }
  | { op: 'frame'; on: boolean; bend: number; wheel: number; gain: number }
  | { op: 'stop' };
export type EngineStatus = 'ready' | 'unavailable' | 'error';

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
const finite = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);

/** The message, cleaned, or null when it is not a well-formed engine-synth message. */
export function parseEngineMessage(data: unknown): EngineMessage | null {
  if (!data || typeof data !== 'object') return null;
  const m = data as Record<string, unknown>;
  if (m.lpd !== 'engine-synth') return null;
  if (m.op === 'stop') return { op: 'stop' };
  if (m.op === 'start') {
    return typeof m.patch === 'string' && m.patch.length > 0 && m.patch.length <= ENGINE_PATCH_MAX ? { op: 'start', patch: m.patch } : null;
  }
  if (m.op === 'frame') {
    if (typeof m.on !== 'boolean' || !finite(m.bend) || !finite(m.wheel) || !finite(m.gain)) return null;
    return { op: 'frame', on: m.on, bend: clamp(m.bend, -1, 1), wheel: clamp(m.wheel, 0, 1), gain: clamp(m.gain, 0, 1) };
  }
  return null;
}

/** A token bucket: `perSecond` tokens a second, at most `burst` saved up. Returns whether one more message may pass. */
export function rateCap(perSecond = ENGINE_RATE, burst = ENGINE_BURST, now: () => number = () => performance.now()): () => boolean {
  let tokens = burst, last = now();
  return () => {
    const t = now();
    tokens = Math.min(burst, tokens + Math.max(0, t - last) * perSecond / 1000);
    last = t;
    if (tokens < 1) return false;
    tokens -= 1;
    return true;
  };
}

export interface EngineHost {
  /** Inside the desktop app: the native voice exists. */
  native: boolean;
  /** `game_engine_start`: resolves once the voice is built and handed to the audio engine. */
  start(patch: string): Promise<unknown>;
  frame(on: boolean, bend: number, wheel: number, gain: number): void;
  stop(): void;
  /** Reply to the game frame with `{lpd: 'engine-synth-status', state, message}`. */
  reply(state: EngineStatus, message?: string): void;
}

/** Messages from the Dustline frame in, voice commands out. One per app. */
export class EngineLink {
  private host: EngineHost;
  private allow: () => boolean;
  private patch: string | null = null;
  private running = false;
  private starting = false;
  private generation = 0;

  constructor(host: EngineHost, now?: () => number) {
    this.host = host;
    this.allow = rateCap(ENGINE_RATE, ENGINE_BURST, now);
  }

  get active(): boolean {
    return this.running;
  }

  /** A message from the prototype frame; `fromDustline`: it came from the active frame and that frame shows Dustline. */
  async receive(data: unknown, fromDustline: boolean): Promise<void> {
    if (!fromDustline || !this.allow()) return;
    const m = parseEngineMessage(data);
    if (!m) return;
    if (m.op === 'stop') return this.stop(true);
    if (m.op === 'start') {
      this.patch = m.patch;
      return this.begin();
    }
    if (this.running) this.host.frame(m.on, m.bend, m.wheel, m.gain);
    // A voice the app stopped while the window was hidden comes back with the next sounding frame.
    else if (m.on && this.patch !== null && !this.starting) await this.begin();
  }

  /** Game switch, leaving GAMES, frame reload: stop and forget the patch. Window hidden: stop only (`forget` false). */
  stop(forget: boolean): void {
    this.generation++;
    this.starting = false;
    if (forget) this.patch = null;
    if (this.running) {
      this.running = false;
      this.host.stop();
    }
  }

  private async begin(): Promise<void> {
    if (!this.host.native) {
      this.host.reply('unavailable');
      return;
    }
    const patch = this.patch;
    if (patch === null) return;
    const generation = ++this.generation;
    this.starting = true;
    try {
      await this.host.start(patch);
      if (generation !== this.generation) {
        if (!this.starting) this.host.stop(); // stopped while the voice was being built
        return;
      }
      this.running = true;
      this.host.reply('ready');
    } catch (error) {
      if (generation === this.generation) this.host.reply('error', error instanceof Error ? error.message : String(error));
    } finally {
      if (generation === this.generation) this.starting = false;
    }
  }
}
