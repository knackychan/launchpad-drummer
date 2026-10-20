// Startup and set-loading progress: pure helpers (the backend owns the truth, this orders and words it).
import type { LoadPhase, LoadStatus, MissingPad, SetModel, Startup } from './model.ts';
import type { SaveStatus } from './status.ts';

/** What the backend reports before anything has happened: request 0, revision 0. */
export const INITIAL_LOAD: LoadStatus = { requestId: 0, revision: 0, phase: 'settings', completed: 0, total: null, failed: 0, error: null };

/** Phases that end a request: nothing more will be reported for it. */
export const isTerminal = (phase: LoadPhase) => phase === 'ready' || phase === 'needsSet' || phase === 'failed';

/**
 * The status to show after `incoming` arrives (from an event or from a snapshot).
 * Events can pass each other, so only a strictly newer revision replaces the current one.
 */
export function acceptLoad(current: LoadStatus | null, incoming: LoadStatus): LoadStatus {
  return current && incoming.revision <= current.revision ? current : incoming;
}

/** Progress bar state: a value only when the total is a real positive number. */
export type Progress = { kind: 'indeterminate' } | { kind: 'determinate'; value: number; max: number };

export function progressOf(status: LoadStatus): Progress {
  const counted = status.phase === 'loadingSamples' || status.phase === 'preparingSamples';
  if (!counted || status.total === null || !Number.isFinite(status.total) || status.total <= 0) return { kind: 'indeterminate' };
  return { kind: 'determinate', value: Math.min(Math.max(0, status.completed), status.total), max: status.total };
}

/** One line for the phase, in the player's words. Never a file path. */
export function phaseLabel(status: LoadStatus): string {
  switch (status.phase) {
    case 'settings': return 'Reading settings…';
    case 'readingSet': return 'Reading set…';
    case 'preparingSamples': return countedLabel('Preparing default samples', status);
    case 'loadingSamples': return countedLabel('Loading samples', status);
    case 'installing': return 'Setting up the set…';
    case 'ready': return 'Ready';
    case 'needsSet': return 'Choose a set';
    case 'failed': return 'The set did not load';
  }
}

function countedLabel(what: string, status: LoadStatus): string {
  const p = progressOf(status);
  return p.kind === 'determinate' ? `${what} — ${p.value} of ${p.max}` : `${what}…`;
}

/** Screen readers hear a phase change, or progress every ~25 % of a counted phase, not every file. */
export function announcement(status: LoadStatus): string {
  const p = progressOf(status);
  if (p.kind === 'indeterminate') return phaseLabel(status);
  const quarter = Math.floor((p.value / p.max) * 4);
  const what = status.phase === 'preparingSamples' ? 'Preparing default samples' : 'Loading samples';
  return `${what} — ${quarter * 25} percent`;
}

/** Seconds of waiting after which the loading view admits it is slow. */
export const SLOW_AFTER_MS = 10_000;
export const isSlow = (elapsedMs: number) => elapsedMs >= SLOW_AFTER_MS;

/**
 * A counter for "is the answer I am waiting for still the one that matters?". Take a token
 * before awaiting; apply the answer only when the token is still current.
 */
export function createGeneration() {
  let current = 0;
  return {
    next: () => ++current,
    bump: () => { current++; },
    isCurrent: (token: number) => token === current,
  };
}

/**
 * A value that arrives both as events and as one fetched snapshot. Once any event was seen the
 * snapshot, which may have been answered earlier, is ignored.
 */
export function eventFirst<T>(apply: (value: T) => void) {
  let eventSeen = false;
  return {
    event: (value: T) => { eventSeen = true; apply(value); },
    snapshot: (value: T) => { if (!eventSeen) apply(value); },
  };
}

// ---- the state fetched once a set is ready ----------------------------------------------------

export interface ReadyCalls {
  /** Required: without the set and the launch outcome there is nothing to show. */
  set: () => Promise<SetModel>;
  startup: () => Promise<Startup>;
  /** Optional: a failure leaves the matching UI state as it was. */
  bank: () => Promise<number>;
  save: () => Promise<SaveStatus>;
  history: () => Promise<{ canUndo: boolean; canRedo: boolean }>;
  missing: () => Promise<MissingPad[]>;
  /** Asked again after the others answered: who owns the backend now? */
  status: () => Promise<LoadStatus>;
}

export type ReadyResult =
  | { kind: 'stale' }
  | { kind: 'error'; error: string }
  | {
      kind: 'ok';
      set: SetModel;
      startup: Startup;
      bank: number | null;
      save: SaveStatus | null;
      history: { canUndo: boolean; canRedo: boolean } | null;
      missing: MissingPad[] | null;
    };

const optional = async <T>(call: () => Promise<T>): Promise<T | null> => {
  try { return (await call()) ?? null; } catch { return null; }
};

const reason = (e: unknown) => String(e instanceof Error ? e.message : e);

/**
 * Fetch everything a ready set needs, all at once. The answer is `stale` when `isCurrent()` turned
 * false while waiting (the user opened something else, another ready notice arrived) or when the
 * backend now belongs to another request than `requestId`: the caller must then apply nothing.
 * An optional request that fails is ignored; a required one makes the result an `error`.
 */
export async function fetchReadyState(calls: ReadyCalls, requestId: number, isCurrent: () => boolean): Promise<ReadyResult> {
  const [set, startup, bank, save, history, missing] = await Promise.allSettled([
    calls.set(), calls.startup(), optional(calls.bank), optional(calls.save), optional(calls.history), optional(calls.missing),
  ]);
  if (!isCurrent()) return { kind: 'stale' };
  // The numbers above may predate a newer request whose event has not reached us yet.
  const owner = await optional(calls.status);
  if (!isCurrent()) return { kind: 'stale' };
  if (owner && (owner.requestId !== requestId || owner.phase !== 'ready')) return { kind: 'stale' };
  if (set.status === 'rejected' || !set.value) return { kind: 'error', error: set.status === 'rejected' ? reason(set.reason) : 'The set did not arrive.' };
  if (startup.status === 'rejected' || !startup.value) return { kind: 'error', error: startup.status === 'rejected' ? reason(startup.reason) : 'The launch state did not arrive.' };
  const value = <T>(r: PromiseSettledResult<T | null>): T | null => (r.status === 'fulfilled' ? r.value : null);
  return { kind: 'ok', set: set.value, startup: startup.value, bank: value(bank), save: value(save), history: value(history), missing: value(missing) };
}
