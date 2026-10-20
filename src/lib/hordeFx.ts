import { cellsOf, type Enemy, type EnemyType, type HordeEvent, type HordeState, type Aids } from './horde.ts';
import { H, W, type ArenaLayout } from './hordeLayout.ts';
import type { HordeAudio } from './hordeAudio.ts';

export const P = { red: '#e63b2e', red2: '#a8261c', teal: '#38b6c5', teal2: '#1f7f8c', cream: '#f3e3b8', cream2: '#d9c38f', sand3: '#b89d68',
  yellow: '#f5c644', yellow2: '#c4931c', amber: '#ffb21a', pink: '#f4a0c0', pink2: '#c9668e', orange: '#f08a2b', orange2: '#b45a12', ink: '#141414', grey: '#6e6f78',
  green: '#5fd38a', green2: '#2c8a52', purple: '#a77be6', purple2: '#6a43a8', white: '#ffffff', foam: '#e9f7f3', ice: '#a8e6ff', blue: '#5aa9ff' };

// Screen colours per enemy: c1 is the body and the LED underglow, c2 the shade.
export const LOOK: Record<EnemyType, { c1: string; c2: string }> = {
  grunt: { c1: P.green, c2: P.green2 }, runner: { c1: P.yellow, c2: P.yellow2 }, brute: { c1: P.orange, c2: P.orange2 },
  splitter: { c1: P.purple, c2: P.purple2 }, survivor: { c1: P.white, c2: P.teal }, boss: { c1: P.red, c2: P.red2 },
};

export type HordeSettings = {
  aids: Aids; quality: 'high' | 'low'; motion: 'full' | 'reduce'; crush: 0 | 3 | 4 | 5; crt: 'on' | 'curved' | 'off'; sound: 'sfx' | 'synth' | 'off';
};
export const DEFAULT_SETTINGS: HordeSettings = { aids: 'medium', quality: 'high', motion: 'full', crush: 4, crt: 'on', sound: 'sfx' };

export type Particle = { x: number; y: number; vx: number; vy: number; t: number; col: string; s: number; spark: boolean };
export type Floater = { x: number; y: number; txt: string; col: string; size: number; t: number; t0: number };
export type Ring = { x: number; y: number; t: number; t0: number; r0: number; r1: number; col: string };
export type Banner = { text: string; sub: string; t: number; t0: number };
export type Fx = {
  parts: Particle[]; floaters: Floater[]; decals: { x: number; y: number; col: string; t: number; s: number; seed: number }[];
  tracers: { x: number; y: number; t: number; t0: number }[]; rings: Ring[]; keyPress: Float32Array;
  shake: number; rankPop: number; turretAng: number; muzzleT: number; hpTrail: number; banner: Banner | null; beatFlash: number;
};

export const keySlot = (column: number, row: number) => row * 12 + column;
const rnd = (a: number, b: number) => a + Math.random() * (b - a);

export function createFx(): Fx {
  return { parts: [], floaters: [], decals: [], tracers: [], rings: [], keyPress: new Float32Array(120), shake: 0, rankPop: 0,
    turretAng: -Math.PI / 2, muzzleT: 0, hpTrail: 100, banner: null, beatFlash: 0 };
}

function waveHint(wave: number): string {
  if (wave % 3 === 0) return 'THE CRAB KING IS COMING';
  return ({ 1: 'HIT THE LIT PADS', 2: 'RUNNERS: FAST', 4: 'BRUTES TAKE 3 HITS · DON\'T SHOOT SURVIVORS', 5: 'SPLITTERS BREAK IN TWO' } as Record<number, string>)[wave]
    ?? 'HOLD THE BEACH';
}

export function updateFx(fx: Fx, state: HordeState, dt: number): void {
  fx.shake = Math.max(0, fx.shake - dt * 30);
  fx.rankPop = Math.max(0, fx.rankPop - dt * 2.8);
  fx.muzzleT = Math.max(0, fx.muzzleT - dt);
  fx.beatFlash = Math.max(0, fx.beatFlash - dt * 5);
  if (state.hp > fx.hpTrail) fx.hpTrail = state.hp;
  else if (state.hp < fx.hpTrail) fx.hpTrail = Math.max(state.hp, fx.hpTrail - dt * 22);
  for (let i = 0; i < fx.keyPress.length; i++) if (fx.keyPress[i] > 0) fx.keyPress[i] = Math.max(0, fx.keyPress[i] - dt);
  for (const p of fx.parts) { p.t -= dt; p.vy += 260 * dt; p.x += p.vx * dt; p.y += p.vy * dt; }
  fx.parts = fx.parts.filter(p => p.t > 0);
  for (const f of fx.floaters) { f.t -= dt; f.y -= 22 * dt; }
  fx.floaters = fx.floaters.filter(f => f.t > 0);
  for (const d of fx.decals) d.t -= dt;
  fx.decals = fx.decals.filter(d => d.t > 0);
  for (const t of fx.tracers) t.t -= dt;
  fx.tracers = fx.tracers.filter(t => t.t > 0);
  for (const r of fx.rings) r.t -= dt;
  fx.rings = fx.rings.filter(r => r.t > 0);
  if (fx.banner && (fx.banner.t -= dt) <= 0) fx.banner = null;
}

// Turns the rule events of one frame into particles, floating text, screen shake and sound.
export function applyEvents(fx: Fx, layout: ArenaLayout, events: readonly HordeEvent[], audio: HordeAudio, settings: HordeSettings): void {
  const burst = (x: number, y: number, col: string, n: number, speed: number, sparks = false) => {
    const count = settings.quality === 'low' ? Math.ceil(n * .5) : n;
    for (let i = 0; i < count; i++) {
      const angle = Math.random() * Math.PI * 2, v = rnd(.3, 1) * speed;
      fx.parts.push({ x, y, vx: Math.cos(angle) * v, vy: Math.sin(angle) * v - 40, t: rnd(.25, .58), col, s: Math.random() < .35 ? 3 : 2, spark: sparks && i % 2 === 0 });
    }
  };
  const floatAt = (x: number, y: number, txt: string, col: string, size: number) => fx.floaters.push({ x, y, txt, col, size, t: .9, t0: .9 });
  const centre = (column: number, row: number) => { const r = layout.rect(column, row); return { x: r.x + r.w / 2, y: r.y + r.h / 2, r }; };
  const turret = layout.turret;

  events.forEach((event, index) => {
    switch (event.kind) {
      case 'start': audio.init(); fx.parts = []; fx.floaters = []; fx.decals = []; fx.tracers = []; fx.rings = []; fx.keyPress.fill(0);
        fx.shake = 0; fx.rankPop = 0; fx.hpTrail = 100; break;
      case 'spawn': audio.spawn(); break;
      case 'wave': fx.banner = { text: `WAVE ${event.wave}`, sub: waveHint(event.wave), t: 2.4, t0: 2.4 }; audio.wave(); break;
      case 'clear': fx.banner = { text: `WAVE ${event.wave} CLEAR`, sub: '+10 HP · NEXT WAVE INCOMING', t: 3, t0: 3 }; audio.wave(); break;
      case 'lose': audio.lose(); fx.shake = 14; break;
      case 'shot': {
        const { x, y } = centre(event.column, event.row);
        fx.keyPress[keySlot(event.column, event.row)] = .18;
        fx.turretAng = Math.atan2(y - turret.y, x - turret.x); fx.muzzleT = .08;
        fx.tracers.push({ x, y, t: .09, t0: .09 });
        audio.shot();
        break;
      }
      case 'hit': {
        const { x, y, r } = centre(event.column, event.row);
        if (event.onBeat) floatAt(x, r.y - 10, 'ON BEAT ×2', P.pink, 10);
        burst(x, y, P.white, 8, 115, true);
        floatAt(x, r.y + 4, `+${event.points}`, P.cream, 9);
        if (events[index + 1]?.kind !== 'kill') audio.hit(event.combo);
        break;
      }
      case 'kill': {
        const boss = event.type === 'boss';
        const cells = cellsOf({ type: event.type, column: event.column, row: event.row } as Enemy);
        const spots = cells.map(([c, r]) => layout.rect(c, r));
        const cx = spots.reduce((s, q) => s + q.x + layout.S / 2, 0) / spots.length, cy = spots.reduce((s, q) => s + q.y + layout.S / 2, 0) / spots.length;
        audio.kill(event.combo);
        burst(cx, cy, LOOK[event.type].c1, boss ? 40 : 16, boss ? 220 : 140, true);
        burst(cx, cy, P.ink, 6, 80);
        fx.decals.push({ x: cx, y: cy, col: LOOK[event.type].c2, t: 5, s: boss ? 1.8 : 1, seed: Math.random() * 9 });
        if (boss) {
          fx.shake = 10;
          fx.rings.push({ x: cx, y: cy, t: .5, t0: .5, r0: 10, r1: 120, col: P.red });
          floatAt(cx, cy - 20, `KING DOWN +${event.bonus}`, P.yellow, 14);
        }
        break;
      }
      case 'miss': { const { x, r } = centre(event.column, event.row); burst(x, r.y + r.h * .7, P.cream2, 8, 60); audio.miss(); break; }
      case 'armour': { const { x, y, r } = centre(event.column, event.row); burst(x, y, P.yellow, 8, 135, true); floatAt(x, r.y + 6, 'ARMOUR', P.grey, 8); audio.clank(); break; }
      case 'friendly': {
        const { x, y, r } = centre(event.column, event.row);
        burst(x, y, P.teal, 12, 100); fx.shake = Math.max(fx.shake, 9);
        floatAt(x, r.y - 8, 'FRIENDLY! -10', P.red, 11); audio.oops();
        break;
      }
      case 'push': { const { x, r } = centre(event.column, event.row); floatAt(x, r.y - 4, 'PUSH', P.orange, 8); break; }
      case 'split': {
        const { x, y } = centre(event.column, event.row);
        fx.rings.push({ x, y, t: .32, t0: .32, r0: 4, r1: layout.S * 1.15, col: P.purple });
        burst(x, y, P.white, 10, 130, true); floatAt(x, y - 10, 'SPLIT!', P.purple, 9);
        break;
      }
      case 'bomb': {
        const { x, y } = centre(event.column, event.row);
        audio.bomb(); fx.shake = 11;
        fx.rings.push({ x, y, t: .48, t0: .48, r0: 6, r1: layout.S * 2.15, col: P.orange });
        fx.rings.push({ x, y, t: .28, t0: .28, r0: 4, r1: layout.S * 1.55, col: P.white });
        burst(x, y, P.orange, 32, 210, true); burst(x, y, P.yellow, 18, 150, true);
        for (let dc = -1; dc <= 1; dc++) for (let dr = -1; dr <= 1; dr++) {
          const c = event.column + dc, r = event.row + dr;
          if (c >= 1 && c <= layout.cols && r >= 1 && r <= layout.rows) fx.keyPress[keySlot(c, r)] = .26;
        }
        break;
      }
      case 'base': {
        const rect = layout.rect(event.column, 1);
        const x = rect.x + (event.type === 'boss' ? layout.S : layout.S / 2), y = layout.base.y + 6;
        audio.base(); burst(x, y, P.cream2, 18, 160, true); burst(x, y, LOOK[event.type].c1, 10, 120);
        floatAt(x, y - 14, `-${event.damage}`, P.red, 13); fx.shake = Math.max(fx.shake, 9);
        break;
      }
      case 'saved': {
        const rect = layout.rect(event.column, 1), x = rect.x + layout.S / 2, y = layout.base.y + 6;
        floatAt(x, y - 10, 'SAVED +5 HP', P.teal, 10); burst(x, y, P.teal, 12, 90, true); audio.saved();
        break;
      }
      case 'arm': audio.arm(); break;
      case 'deny': audio.deny(); if (event.reason === 'bombs') floatAt(turret.x, turret.y - 40, 'NO BOMBS', P.grey, 10); break;
      case 'freeze': audio.freeze(); fx.rings.push({ x: W / 2, y: H / 2, t: .6, t0: .6, r0: 10, r1: 420, col: P.ice }); break;
      case 'bombplus': floatAt(turret.x, turret.y - 52, 'BOMB +1', P.orange, 11); break;
      case 'rank': fx.rankPop = 1; audio.rank(); floatAt(turret.x, turret.y - 40, `RANK ${event.rank}!`, P.yellow, 14); break;
      case 'comboLost': floatAt(turret.x, turret.y - 30, event.why, P.red, 10); break;
      case 'beat': {
        fx.beatFlash = 1;
        const slot = (event.count - 1) % 4;
        if (slot === 0 || slot === 2) audio.kick(); else audio.snare();
        audio.tick();
        break;
      }
    }
  });
}
