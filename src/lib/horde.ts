import { createLedFrame, padNote, type Rgb } from './led.ts';

export type EnemyType = 'grunt' | 'runner' | 'brute' | 'splitter' | 'survivor' | 'boss';
export type Enemy = { id: number; type: EnemyType; column: number; row: number; hp: number; stepIn: number; weak?: number; beats?: number; moves?: number };
export type HordeState = {
  mode: 'title' | 'play' | 'over'; wave: number; hp: number; score: number; combo: number; best: number;
  kills: number; saved: number; enemies: Enemy[]; queue: EnemyType[]; spawnIn: number; nextId: number; time: number;
  playMode: 'free' | 'beat'; comboIn: number; bombs: number; armed: boolean; charge: number; freezeIn: number;
  beatClock: number; beatCount: number;
};

export const ENEMY: Record<EnemyType, { hp: number; step: number; score: number; color: Rgb }> = {
  grunt: { hp: 1, step: 1.4, score: 20, color: [8, 63, 8] },
  runner: { hp: 1, step: .7, score: 30, color: [63, 50, 0] },
  brute: { hp: 3, step: 2.1, score: 60, color: [63, 18, 0] },
  splitter: { hp: 2, step: 1.6, score: 40, color: [34, 0, 63] },
  survivor: { hp: 1, step: 1.5, score: 0, color: [40, 63, 63] },
  boss: { hp: 12, step: 3.2, score: 400, color: [63, 0, 0] },
};
const BEATS: Record<EnemyType, number> = { grunt: 2, runner: 1, brute: 3, splitter: 2, survivor: 2, boss: 4 };
const beatLength = (state: HordeState) => 60 / Math.min(168, 96 + 5 * (state.wave - 1));
const multiplier = (state: HordeState) => Math.min(8, 1 + Math.floor(state.combo / 5));
const cells = (enemy: Enemy) => enemy.type === 'boss'
  ? [[enemy.column, enemy.row], [enemy.column + 1, enemy.row], [enemy.column, enemy.row + 1], [enemy.column + 1, enemy.row + 1]]
  : [[enemy.column, enemy.row]];
const enemyAt = (state: HordeState, column: number, row: number) =>
  state.enemies.find(enemy => cells(enemy).some(([c, r]) => c === column && r === row));
const breakCombo = (state: HordeState) => { state.combo = 0; state.comboIn = 0; };

export function createHorde(seed = Date.now()): HordeState {
  return { mode: 'title', wave: 0, hp: 100, score: 0, combo: 0, best: 0, kills: 0, saved: 0,
    enemies: [], queue: [], spawnIn: 0, nextId: 1, time: seed % 1_000_000,
    playMode: 'free', comboIn: 0, bombs: 1, armed: false, charge: 0, freezeIn: 0, beatClock: 0, beatCount: 0 };
}

export function startHorde(state: HordeState): void {
  Object.assign(state, { mode: 'play', wave: 1, hp: 100, score: 0, combo: 0, best: 0, kills: 0, saved: 0,
    enemies: [], queue: waveQueue(1), spawnIn: state.playMode === 'beat' ? 3 : 1.6, nextId: 1,
    comboIn: 0, bombs: 1, armed: false, charge: 0, freezeIn: 0, beatClock: 0, beatCount: 0 });
}

function waveQueue(wave: number): EnemyType[] {
  const pool: EnemyType[] = ['grunt', 'grunt', 'grunt', 'grunt', 'grunt', 'grunt'];
  if (wave >= 2) pool.push('runner', 'runner', 'runner');
  if (wave >= 4) pool.push('brute', 'brute', 'survivor');
  if (wave >= 5) pool.push('splitter', 'splitter');
  if (wave === 3) pool.push('runner', 'runner');
  const count = 6 + 4 * wave;
  const queue = Array.from({ length: count }, (_, i) => pool[(i * 5 + wave) % pool.length]);
  if (wave % 3 === 0) queue.push('boss');
  return queue;
}

export function hitCell(state: HordeState, column: number, row: number): { hit: boolean; killed: boolean } {
  if (state.mode !== 'play') { startHorde(state); return { hit: false, killed: false }; }
  if (column === 9) {
    if (row === 8 && state.bombs > 0) state.armed = !state.armed;
    if (row === 7 && state.charge >= 100 && state.freezeIn <= 0) { state.charge = 0; state.freezeIn = 4; }
    return { hit: false, killed: false };
  }
  if (column < 1 || column > 8 || row < 1 || row > 8) return { hit: false, killed: false };
  const onBeat = state.playMode === 'beat' && Math.min(state.beatClock, beatLength(state) - state.beatClock) < .1;
  if (state.armed) {
    state.armed = false; state.bombs--;
    const targets = [...state.enemies].filter(enemy => cells(enemy).some(([c, r]) => Math.abs(c - column) <= 1 && Math.abs(r - row) <= 1));
    let hit = false, killed = false;
    for (const enemy of targets) {
      if (enemy.type === 'survivor') friendlyFire(state, enemy);
      else { hit = true; killed = damage(state, enemy, 2, onBeat) || killed; }
    }
    if (!hit) breakCombo(state);
    return { hit, killed };
  }
  const enemy = enemyAt(state, column, row);
  if (!enemy) { breakCombo(state); return { hit: false, killed: false }; }
  if (enemy.type === 'survivor') { friendlyFire(state, enemy); return { hit: true, killed: false }; }
  if (enemy.type === 'boss') {
    const [weakColumn, weakRow] = cells(enemy)[enemy.weak ?? 0];
    if (column !== weakColumn || row !== weakRow) return { hit: true, killed: false };
  }
  return { hit: true, killed: damage(state, enemy, 1, onBeat) };
}

function friendlyFire(state: HordeState, enemy: Enemy) {
  state.enemies = state.enemies.filter(item => item !== enemy);
  state.hp = Math.max(0, state.hp - 10);
  breakCombo(state);
  if (state.hp === 0) state.mode = 'over';
}

function damage(state: HordeState, enemy: Enemy, amount: number, onBeat: boolean): boolean {
  enemy.hp -= amount;
  state.combo++;
  state.comboIn = 3;
  state.best = Math.max(state.best, state.combo);
  const bonus = onBeat ? 2 : 1;
  state.score += 10 * multiplier(state) * bonus;
  if (enemy.hp > 0) {
    if (enemy.type === 'brute') enemy.row = Math.min(8, enemy.row + 1);
    if (enemy.type === 'boss') enemy.weak = ((enemy.weak ?? 0) + 1) % 4;
    return false;
  }
  state.enemies = state.enemies.filter(item => item !== enemy);
  state.kills++;
  state.score += ENEMY[enemy.type].score * multiplier(state) * bonus;
  state.charge = Math.min(100, state.charge + 7);
  if (state.kills % 20 === 0) state.bombs = Math.min(3, state.bombs + 1);
  if (enemy.type === 'splitter') for (const offset of [-1, 1]) {
    const column = enemy.column + offset;
    if (column >= 1 && column <= 8 && !enemyAt(state, column, enemy.row))
      state.enemies.push({ id: state.nextId++, type: 'grunt', column, row: enemy.row, hp: 1, stepIn: ENEMY.grunt.step });
  }
  return true;
}

export function tickHorde(state: HordeState, dt: number): void {
  if (!Number.isFinite(dt) || dt <= 0) return;
  state.time += dt;
  if (state.mode !== 'play') return;
  if (state.comboIn > 0 && (state.comboIn -= dt) <= 0) breakCombo(state);
  const frozen = state.freezeIn > 0;
  state.freezeIn = Math.max(0, state.freezeIn - dt);
  if (state.playMode === 'beat') {
    state.beatClock += dt;
    const length = beatLength(state);
    while (state.beatClock >= length) {
      state.beatClock -= length; state.beatCount++;
      if (state.queue.length && --state.spawnIn <= 0)
        state.spawnIn = spawn(state) ? Math.max(1, Math.round((1.45 - .1 * state.wave) / length)) : 1;
      if (!frozen) for (const enemy of [...state.enemies]) {
        enemy.beats = (enemy.beats ?? 0) + 1;
        if (enemy.beats >= BEATS[enemy.type]) { enemy.beats = 0; stepEnemy(state, enemy); }
      }
    }
  } else {
    state.spawnIn -= dt;
    if (state.queue.length && state.spawnIn <= 0)
      state.spawnIn = spawn(state) ? Math.max(.38, 1.45 - .1 * state.wave) : .2;
    if (!frozen) for (const enemy of [...state.enemies]) {
      enemy.stepIn -= dt * (1 + .07 * (state.wave - 1));
      while (enemy.stepIn <= 0 && state.enemies.includes(enemy)) {
        stepEnemy(state, enemy);
        enemy.stepIn += ENEMY[enemy.type].step;
      }
    }
  }
  if (state.hp === 0) { state.mode = 'over'; return; }
  if (!state.queue.length && !state.enemies.length) {
    state.wave++;
    state.hp = Math.min(100, state.hp + 10);
    state.queue = waveQueue(state.wave);
    state.spawnIn = state.playMode === 'beat' ? 3 : 1.6;
  }
}

function spawn(state: HordeState): boolean {
  const type = state.queue[0];
  const row = type === 'boss' ? 7 : 8;
  const maxColumn = type === 'boss' ? 7 : 8;
  const start = (state.nextId * 5) % maxColumn;
  let column = 0;
  for (let offset = 0; offset < maxColumn; offset++) {
    const candidate = (start + offset) % maxColumn + 1;
    if (cells({ id: 0, type, column: candidate, row, hp: 1, stepIn: 0 })
      .every(([c, r]) => !enemyAt(state, c, r))) { column = candidate; break; }
  }
  if (!column) return false;
  state.queue.shift();
  const id = state.nextId++;
  state.enemies.push({ id, type, column, row: type === 'boss' ? 7 : 8,
    hp: ENEMY[type].hp, stepIn: ENEMY[type].step, weak: type === 'boss' ? 0 : undefined, beats: 0 });
  return true;
}

function stepEnemy(state: HordeState, enemy: Enemy) {
  if (enemy.row === 1) {
    state.enemies = state.enemies.filter(item => item !== enemy);
    if (enemy.type === 'survivor') { state.saved++; state.hp = Math.min(100, state.hp + 5); state.score += 100; }
    else { state.hp = Math.max(0, state.hp - (enemy.type === 'brute' ? 15 : enemy.type === 'boss' ? 35 : 6)); breakCombo(state); }
    return;
  }
  const nextRow = enemy.row - 1;
  enemy.moves = (enemy.moves ?? 0) + 1;
  const nextColumn = enemy.type === 'runner' && (enemy.moves + enemy.id) % 3 === 0
    ? Math.max(1, Math.min(8, enemy.column + (enemy.id % 2 ? 1 : -1))) : enemy.column;
  const open = (column: number) => cells({ ...enemy, column, row: nextRow })
    .every(([c, r]) => c >= 1 && c <= 8 && r <= 8 && state.enemies.every(other => other === enemy || !cells(other).some(([oc, or]) => oc === c && or === r)));
  if (open(nextColumn)) { enemy.column = nextColumn; enemy.row = nextRow; }
  else if (nextColumn !== enemy.column && open(enemy.column)) enemy.row = nextRow;
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
  for (const enemy of state.enemies) for (const [column, row] of cells(enemy)) {
    frame[(8 - row) * 8 + column - 1] = ENEMY[enemy.type].color;
  }
  for (let c = 0; c < 8; c++) if (c < Math.round(state.hp / 100 * 8) && frame[c].every(channel => channel === 0))
    frame[c] = state.hp > 50 ? [0, 36, 8] : state.hp > 25 ? [40, 28, 0] : [48, 0, 0];
  return frame;
}

export function noteCell(note: number): { column: number; row: number } {
  return { column: note % 10, row: Math.floor(note / 10) };
}

export function cellNote(column: number, row: number): number { return padNote(row, column); }
