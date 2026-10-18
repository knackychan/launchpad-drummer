import { createLedFrame, padNote, type Rgb } from './led.ts';

export type EnemyType = 'grunt' | 'runner' | 'brute' | 'splitter' | 'survivor' | 'boss';
export type Enemy = { id: number; type: EnemyType; column: number; row: number; hp: number; stepIn: number };
export type HordeState = {
  mode: 'title' | 'play' | 'over'; wave: number; hp: number; score: number; combo: number; best: number;
  kills: number; saved: number; enemies: Enemy[]; queue: EnemyType[]; spawnIn: number; nextId: number; time: number;
};

export const ENEMY: Record<EnemyType, { hp: number; step: number; score: number; color: Rgb }> = {
  grunt: { hp: 1, step: 1.4, score: 20, color: [8, 63, 8] },
  runner: { hp: 1, step: .7, score: 30, color: [63, 50, 0] },
  brute: { hp: 3, step: 2.1, score: 60, color: [63, 18, 0] },
  splitter: { hp: 2, step: 1.6, score: 40, color: [34, 0, 63] },
  survivor: { hp: 1, step: 1.5, score: 0, color: [40, 63, 63] },
  boss: { hp: 12, step: 3.2, score: 400, color: [63, 0, 0] },
};

export function createHorde(seed = Date.now()): HordeState {
  return { mode: 'title', wave: 0, hp: 100, score: 0, combo: 0, best: 0, kills: 0, saved: 0,
    enemies: [], queue: [], spawnIn: 0, nextId: 1, time: seed % 1_000_000 };
}

export function startHorde(state: HordeState): void {
  Object.assign(state, { mode: 'play', wave: 1, hp: 100, score: 0, combo: 0, best: 0, kills: 0, saved: 0,
    enemies: [], queue: waveQueue(1), spawnIn: 1.6, nextId: 1 });
}

function waveQueue(wave: number): EnemyType[] {
  const pool: EnemyType[] = ['grunt', 'grunt', 'grunt', 'grunt', 'grunt', 'grunt'];
  if (wave >= 2) pool.push('runner', 'runner', 'runner');
  if (wave >= 4) pool.push('brute', 'brute', 'survivor');
  if (wave >= 5) pool.push('splitter', 'splitter');
  if (wave === 3) pool.push('runner', 'runner');
  const count = 6 + 4 * wave;
  const queue = Array.from({ length: count }, (_, i) => pool[(i * 7 + wave) % pool.length]);
  if (wave % 3 === 0) queue.push('boss');
  return queue;
}

export function hitCell(state: HordeState, column: number, row: number): { hit: boolean; killed: boolean } {
  if (state.mode === 'title') { startHorde(state); return { hit: false, killed: false }; }
  if (state.mode !== 'play') return { hit: false, killed: false };
  const enemy = state.enemies.find(item => item.column === column && item.row === row);
  if (!enemy) { state.combo = 0; return { hit: false, killed: false }; }
  if (enemy.type === 'survivor') { state.hp = Math.max(0, state.hp - 10); state.combo = 0; }
  else {
    state.combo++;
    state.best = Math.max(state.best, state.combo);
    enemy.hp--;
    if (enemy.hp <= 0) {
      state.enemies = state.enemies.filter(item => item !== enemy);
      state.kills++;
      state.score += ENEMY[enemy.type].score * Math.min(8, 1 + Math.floor(state.combo / 5));
      return { hit: true, killed: true };
    }
    if (enemy.type === 'brute') enemy.row = Math.min(8, enemy.row + 1);
  }
  return { hit: true, killed: false };
}

export function tickHorde(state: HordeState, dt: number): void {
  if (!Number.isFinite(dt) || dt <= 0) return;
  state.time += dt;
  if (state.mode !== 'play') return;
  state.spawnIn -= dt;
  if (state.queue.length && state.spawnIn <= 0) {
    const type = state.queue.shift()!;
    state.enemies.push({ id: state.nextId++, type, column: ((state.nextId * 5) % 8) + 1, row: 8, hp: ENEMY[type].hp, stepIn: ENEMY[type].step });
    state.spawnIn = Math.max(.38, 1.45 - .1 * state.wave);
  }
  for (const enemy of state.enemies) {
    enemy.stepIn -= dt;
    while (enemy.stepIn <= 0 && enemy.row > 1) { enemy.row--; enemy.stepIn += ENEMY[enemy.type].step; }
    if (enemy.row === 1 && enemy.stepIn <= 0) {
      state.hp = Math.max(0, state.hp - (enemy.type === 'brute' ? 15 : enemy.type === 'boss' ? 35 : 6));
      state.enemies = state.enemies.filter(item => item !== enemy);
      state.combo = 0;
    }
  }
  if (state.hp === 0) state.mode = 'over';
  if (!state.queue.length && !state.enemies.length) {
    state.wave++;
    state.hp = Math.min(100, state.hp + 10);
    state.queue = waveQueue(state.wave);
    state.spawnIn = 1.6;
  }
}

export function ledFrame(state: HordeState, time = state.time): Rgb[] {
  const frame = createLedFrame();
  if (state.mode === 'title') {
    for (let column = 1; column <= 8; column++) {
      const row = Math.max(1, 8 - Math.floor((time * 3 + column * 1.7) % 8));
      frame[(8 - row) * 8 + column - 1] = column % 3 === 0 ? ENEMY.runner.color : ENEMY.grunt.color;
    }
    return frame;
  }
  for (const enemy of state.enemies) {
    const idx = (8 - enemy.row) * 8 + enemy.column - 1;
    frame[idx] = enemy.type === 'brute' || enemy.type === 'splitter' || enemy.type === 'boss'
      ? ENEMY[enemy.type].color : ENEMY[enemy.type].color;
  }
  for (let c = 0; c < 8; c++) if (c < Math.round(state.hp / 100 * 8)) frame[c] = state.hp > 50 ? [0, 36, 8] : state.hp > 25 ? [40, 28, 0] : [48, 0, 0];
  return frame;
}

export function noteCell(note: number): { column: number; row: number } {
  return { column: note % 10, row: Math.floor(note / 10) };
}

export function cellNote(column: number, row: number): number { return padNote(row, column); }
