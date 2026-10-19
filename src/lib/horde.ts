import { createLedFrame, padNote, type Rgb } from './led.ts';

export type EnemyType = 'grunt' | 'runner' | 'brute' | 'splitter' | 'survivor' | 'boss';
export type Layout = 'pad' | 'keys';
export type PlayMode = 'free' | 'beat';
export type Aids = 'full' | 'medium' | 'low';
export type Enemy = {
  id: number; type: EnemyType; column: number; row: number; hp: number; stepIn: number;
  weak?: number; beats?: number;
  // Entering from the sea: 'gate' sits one row above the arena until it walks in. Missing means 'walk'.
  state?: 'gate' | 'walk'; gateIn?: number; gateBeats?: number;
  // Presentation timers and previous cell, advanced here so the Launchpad lights and the canvas agree.
  max?: number; pc?: number; pr?: number; moveT?: number; hitT?: number; squashT?: number; wob?: number; face?: number; weakT?: number;
};
export type CellFx = { column: number; row: number; kind: 'hit' | 'miss' | 'bomb' | 'armour'; t: number; t0: number };
export type HordeEvent =
  | { kind: 'start' } | { kind: 'spawn' } | { kind: 'wave'; wave: number } | { kind: 'clear'; wave: number } | { kind: 'lose' }
  | { kind: 'shot'; column: number; row: number }
  | { kind: 'hit'; column: number; row: number; points: number; combo: number; onBeat: boolean }
  | { kind: 'kill'; type: EnemyType; column: number; row: number; bonus: number; combo: number }
  | { kind: 'miss'; column: number; row: number } | { kind: 'armour'; column: number; row: number }
  | { kind: 'friendly'; column: number; row: number } | { kind: 'push'; column: number; row: number }
  | { kind: 'split'; column: number; row: number } | { kind: 'bomb'; column: number; row: number }
  | { kind: 'base'; type: EnemyType; column: number; row: number; damage: number }
  | { kind: 'saved'; column: number; row: number }
  | { kind: 'arm' } | { kind: 'freeze' } | { kind: 'deny'; reason: 'bombs' | 'freeze' | 'disarm' } | { kind: 'bombplus' }
  | { kind: 'rank'; rank: string } | { kind: 'comboLost'; why: string; combo: number } | { kind: 'beat'; count: number };
export type HordeState = {
  mode: 'title' | 'play' | 'inter' | 'over'; wave: number; hp: number; score: number; combo: number; best: number;
  kills: number; saved: number; enemies: Enemy[]; queue: EnemyType[]; spawnIn: number; nextId: number; time: number;
  playMode: PlayMode; layout: Layout; speed: number; comboIn: number; bombs: number; armed: boolean; charge: number; freezeIn: number;
  beatClock: number; beatCount: number;
  modeT: number; paused: boolean; waveTotal: number; waveDone: number; shots: number; hits: number; perfect: number;
  hurtT: number; cellFx: CellFx[]; events: HordeEvent[]; seed: number; onBeatShot: boolean; beatBonus: boolean;
};

export const ENEMY: Record<EnemyType, { hp: number; step: number; dmg: number; score: number; color: Rgb; name: string }> = {
  grunt: { hp: 1, step: 1.4, dmg: 6, score: 20, color: [8, 63, 8], name: 'GRUNT' },
  runner: { hp: 1, step: .7, dmg: 5, score: 30, color: [63, 50, 0], name: 'RUNNER' },
  brute: { hp: 3, step: 2.1, dmg: 15, score: 60, color: [63, 18, 0], name: 'BRUTE' },
  splitter: { hp: 2, step: 1.6, dmg: 8, score: 40, color: [34, 0, 63], name: 'SPLITTER' },
  survivor: { hp: 1, step: 1.5, dmg: 0, score: 0, color: [40, 63, 63], name: 'SURVIVOR' },
  boss: { hp: 12, step: 3.2, dmg: 35, score: 400, color: [63, 0, 0], name: 'CRAB KING' },
};
export const BEATS: Record<EnemyType, number> = { grunt: 2, runner: 1, brute: 3, splitter: 2, survivor: 2, boss: 4 };
const ON_BEAT = .1;
const GATE_SECONDS = { single: 1.1, boss: 2.2 };

export const columnsOf = (state: Pick<HordeState, 'layout'>) => state.layout === 'keys' ? 10 : 8;
export const rowsOf = (state: Pick<HordeState, 'layout'>) => state.layout === 'keys' ? 4 : 8;
const stepScale = (state: HordeState) => state.layout === 'keys' ? 1.55 : 1;
export const stepSeconds = (state: HordeState, type: EnemyType) => ENEMY[type].step * stepScale(state);
export const beatsPerMinute = (state: HordeState) =>
  Math.min(168, (96 + 5 * (state.wave - 1)) * state.speed * (state.layout === 'keys' ? .8 : 1));
const beatLength = (state: HordeState) => 60 / beatsPerMinute(state);
export const multiplier = (state: Pick<HordeState, 'combo'>) => Math.min(8, 1 + Math.floor(state.combo / 5));
export const rankOf = (combo: number) => combo >= 35 ? 'S' : combo >= 20 ? 'A' : combo >= 10 ? 'B' : combo >= 5 ? 'C' : 'D';
const spawnGap = (state: HordeState) => Math.max(.38, 1.45 - .1 * state.wave) / state.speed;

export const cellsOf = (enemy: Enemy) => enemy.type === 'boss'
  ? [[enemy.column, enemy.row], [enemy.column + 1, enemy.row], [enemy.column, enemy.row + 1], [enemy.column + 1, enemy.row + 1]]
  : [[enemy.column, enemy.row]];
const isWalking = (enemy: Enemy) => enemy.state !== 'gate';
export const enemyAt = (state: HordeState, column: number, row: number) =>
  state.enemies.find(enemy => isWalking(enemy) && cellsOf(enemy).some(([c, r]) => c === column && r === row));

function free(state: HordeState, column: number, row: number, size: number, self: Enemy | null): boolean {
  for (let dc = 0; dc < size; dc++) for (let dr = 0; dr < size; dr++) {
    const c = column + dc, r = row + dr;
    if (c < 1 || c > columnsOf(state) || r < 1) return false;
    if (r > rowsOf(state)) continue;
    const other = enemyAt(state, c, r);
    if (other && other !== self) return false;
  }
  return true;
}

function rand(state: HordeState): number {
  let t = (state.seed = (state.seed + 0x6D2B79F5) | 0);
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
const between = (state: HordeState, a: number, b: number) => a + rand(state) * (b - a);

function emit(state: HordeState, event: HordeEvent) {
  if (state.events.length < 256) state.events.push(event);
}
export function drainEvents(state: HordeState): HordeEvent[] {
  return state.events.splice(0, state.events.length);
}

function freshRun() {
  return { wave: 0, hp: 100, score: 0, combo: 0, best: 0, kills: 0, saved: 0, enemies: [] as Enemy[], queue: [] as EnemyType[],
    spawnIn: 0, nextId: 1, comboIn: 0, bombs: 1, armed: false, charge: 0, freezeIn: 0, beatClock: 0, beatCount: 0,
    modeT: 0, paused: false, waveTotal: 0, waveDone: 0, shots: 0, hits: 0, perfect: 0, hurtT: 0, cellFx: [] as CellFx[],
    onBeatShot: false, beatBonus: false };
}

export function createHorde(seed = Date.now()): HordeState {
  return { mode: 'title', playMode: 'free', layout: 'pad', speed: 1, time: seed % 1_000_000,
    events: [], seed: (seed >>> 0) || 1, ...freshRun() };
}

// Back to the title screen. Changing the arena, mode or speed always goes through here.
export function resetHorde(state: HordeState, patch: Partial<Pick<HordeState, 'layout' | 'playMode' | 'speed'>> = {}): void {
  Object.assign(state, freshRun(), patch, { mode: 'title' as const });
}

export function startHorde(state: HordeState): void {
  Object.assign(state, freshRun(), { mode: 'play' as const });
  emit(state, { kind: 'start' });
  startWave(state, 1);
}

export function startWave(state: HordeState, wave: number): void {
  state.wave = wave;
  state.queue = waveQueue(state, wave);
  state.waveTotal = state.queue.length;
  state.waveDone = 0;
  state.spawnIn = state.playMode === 'beat' ? 3 : 1.6;
  emit(state, { kind: 'wave', wave });
}

function waveQueue(state: HordeState, wave: number): EnemyType[] {
  const pool: [EnemyType, number][] = [['grunt', 6]];
  if (wave >= 2) pool.push(['runner', 3]);
  if (wave >= 4) pool.push(['brute', 2], ['survivor', 1]);
  if (wave >= 5) pool.push(['splitter', 2]);
  if (wave === 3) pool.push(['runner', 2]);
  const total = pool.reduce((sum, [, weight]) => sum + weight, 0);
  const queue = Array.from({ length: 6 + 4 * wave }, (): EnemyType => {
    let roll = rand(state) * total;
    for (const [type, weight] of pool) if ((roll -= weight) < 0) return type;
    return 'grunt';
  });
  if (wave % 3 === 0) queue.push('boss');
  return queue;
}

export function togglePause(state: HordeState): void {
  if (state.mode === 'play') state.paused = !state.paused;
}

// ---- input ----

type Shot = { hit: boolean; killed: boolean };
const NO_SHOT: Shot = { hit: false, killed: false };

// Title and game-over screens: any pad starts a run (game over waits 1.2 s so a last mash does not skip the results).
function beginRun(state: HordeState): Shot {
  if (state.mode === 'title' || (state.mode === 'over' && state.modeT > 1.2)) startHorde(state);
  return NO_SHOT;
}

function sideButton(state: HordeState, row: number) {
  if (row === 8) armBomb(state);
  else if (row === 7) freezeEnemies(state);
}

// A game cell as the arena sees it (keys layout: columns 1-10, rows 1-4). On the pad layout column 9 holds the side buttons.
export function hitCell(state: HordeState, column: number, row: number): Shot {
  if (state.mode !== 'play') return beginRun(state);
  if (state.layout === 'pad' && column === 9) { sideButton(state, row); return NO_SHOT; }
  return shoot(state, column, row);
}

// A physical Launchpad pad (column 1-9, row 1-8). The keyboard arena lives on the top four pad rows.
export function padPress(state: HordeState, column: number, row: number): Shot {
  if (state.mode !== 'play') return (column <= 8 && row <= 8) || state.mode === 'over' ? beginRun(state) : NO_SHOT;
  if (column === 9) { sideButton(state, row); return NO_SHOT; }
  if (column < 1 || column > 8 || row < 1 || row > 8) return NO_SHOT;
  if (state.layout === 'pad') return shoot(state, column, row);
  return row >= 5 ? shoot(state, column, row - 4) : NO_SHOT;
}

export function armBomb(state: HordeState): void {
  if (state.mode !== 'play') return;
  if (state.armed) { state.armed = false; emit(state, { kind: 'deny', reason: 'disarm' }); return; }
  if (state.bombs <= 0) { emit(state, { kind: 'deny', reason: 'bombs' }); return; }
  state.armed = true;
  emit(state, { kind: 'arm' });
}

// The keyboard arena arms the bomb only while Shift is held.
export function disarmBomb(state: HordeState): void { state.armed = false; }

export function freezeEnemies(state: HordeState): void {
  if (state.mode !== 'play') return;
  if (state.charge < 100 || state.freezeIn > 0) { emit(state, { kind: 'deny', reason: 'freeze' }); return; }
  state.charge = 0; state.freezeIn = 4;
  emit(state, { kind: 'freeze' });
}

function shoot(state: HordeState, column: number, row: number): Shot {
  if (state.mode !== 'play' || state.paused) return NO_SHOT;
  if (column < 1 || column > columnsOf(state) || row < 1 || row > rowsOf(state)) return NO_SHOT;
  state.shots++;
  emit(state, { kind: 'shot', column, row });
  state.onBeatShot = state.playMode === 'beat' && Math.min(state.beatClock, beatLength(state) - state.beatClock) < ON_BEAT;
  if (state.armed) { state.armed = false; state.bombs--; return blast(state, column, row); }
  const enemy = enemyAt(state, column, row);
  if (!enemy) {
    state.cellFx.push({ column, row, kind: 'miss', t: .36, t0: .36 });
    emit(state, { kind: 'miss', column, row });
    breakCombo(state);
    return NO_SHOT;
  }
  if (enemy.type === 'survivor') { state.hits++; friendlyFire(state, enemy, column, row); return { hit: true, killed: false }; }
  if (enemy.type === 'boss') {
    const [weakColumn, weakRow] = cellsOf(enemy)[enemy.weak ?? 0];
    if (column !== weakColumn || row !== weakRow) {
      state.cellFx.push({ column, row, kind: 'armour', t: .22, t0: .22 });
      emit(state, { kind: 'armour', column, row });
      return { hit: true, killed: false };
    }
  }
  state.hits++;
  return { hit: true, killed: damage(state, enemy, 1, column, row) };
}

function blast(state: HordeState, column: number, row: number): Shot {
  emit(state, { kind: 'bomb', column, row });
  const targets = new Set<Enemy>();
  for (let dc = -1; dc <= 1; dc++) for (let dr = -1; dr <= 1; dr++) {
    const c = column + dc, r = row + dr;
    if (c < 1 || c > columnsOf(state) || r < 1 || r > rowsOf(state)) continue;
    state.cellFx.push({ column: c, row: r, kind: 'bomb', t: .48, t0: .48 });
    const enemy = enemyAt(state, c, r);
    if (enemy) targets.add(enemy);
  }
  let hit = false, killed = false;
  for (const enemy of targets) {
    if (enemy.type === 'survivor') { friendlyFire(state, enemy, enemy.column, enemy.row); continue; }
    hit = true;
    const [c, r] = cellsOf(enemy)[0];
    killed = damage(state, enemy, Math.min(2, enemy.hp), c, r) || killed;
  }
  if (hit) state.hits++; else breakCombo(state);
  return { hit, killed };
}

// ---- scoring ----

function breakCombo(state: HordeState, why = 'COMBO LOST') {
  if (state.combo >= 5) emit(state, { kind: 'comboLost', why, combo: state.combo });
  state.combo = 0; state.comboIn = 0;
}

function addCombo(state: HordeState) {
  const before = rankOf(state.combo);
  state.combo++; state.comboIn = 3;
  state.best = Math.max(state.best, state.combo);
  if (rankOf(state.combo) !== before) emit(state, { kind: 'rank', rank: rankOf(state.combo) });
}

function hurt(state: HordeState, amount: number) {
  state.hp = Math.max(0, state.hp - amount);
  state.hurtT = .45;
  if (state.hp <= 0 && state.mode !== 'over') { state.mode = 'over'; state.modeT = 0; emit(state, { kind: 'lose' }); }
}

function friendlyFire(state: HordeState, enemy: Enemy, column: number, row: number) {
  state.enemies = state.enemies.filter(item => item !== enemy);
  state.waveDone++;
  state.cellFx.push({ column, row, kind: 'miss', t: .5, t0: .5 });
  emit(state, { kind: 'friendly', column, row });
  hurt(state, 10);
  breakCombo(state);
}

function damage(state: HordeState, enemy: Enemy, amount: number, column: number, row: number): boolean {
  enemy.hp -= amount; enemy.hitT = .2; enemy.squashT = enemy.type === 'brute' ? .34 : .18;
  addCombo(state);
  const onBeat = state.onBeatShot;
  const points = 10 * multiplier(state) * (onBeat ? 2 : 1);
  state.score += points;
  if (onBeat) { state.perfect++; state.onBeatShot = false; state.beatBonus = true; }
  state.cellFx.push({ column, row, kind: 'hit', t: .28, t0: .28 });
  emit(state, { kind: 'hit', column, row, points, combo: state.combo, onBeat });
  if (enemy.hp <= 0) { kill(state, enemy); return true; }
  const rows = rowsOf(state);
  if (enemy.type === 'brute' && enemy.row < rows && free(state, enemy.column, enemy.row + 1, 1, enemy)) {
    enemy.pc = enemy.column; enemy.pr = enemy.row; enemy.row++; enemy.moveT = 0; enemy.squashT = .38;
    enemy.stepIn = stepSeconds(state, 'brute');
    emit(state, { kind: 'push', column: enemy.column, row: enemy.row });
  }
  if (enemy.type === 'boss') { enemy.weak = ((enemy.weak ?? 0) + 1 + Math.floor(rand(state) * 3)) % 4; enemy.weakT = 0; }
  return false;
}

function kill(state: HordeState, enemy: Enemy) {
  state.enemies = state.enemies.filter(item => item !== enemy);
  state.kills++; state.waveDone++;
  const bonus = ENEMY[enemy.type].score * multiplier(state) * (state.beatBonus ? 2 : 1);
  state.beatBonus = false;
  state.score += bonus;
  emit(state, { kind: 'kill', type: enemy.type, column: enemy.column, row: enemy.row, bonus, combo: state.combo });
  state.charge = Math.min(100, state.charge + 7);
  if (state.kills % 20 === 0 && state.bombs < 3) { state.bombs++; emit(state, { kind: 'bombplus' }); }
  if (enemy.type !== 'splitter') return;
  emit(state, { kind: 'split', column: enemy.column, row: enemy.row });
  for (const offset of [-1, 1]) {
    const column = enemy.column + offset;
    if (column < 1 || column > columnsOf(state) || !free(state, column, enemy.row, 1, null)) continue;
    state.enemies.push({ id: state.nextId++, type: 'grunt', column, row: enemy.row, hp: 1, max: 1,
      stepIn: stepSeconds(state, 'grunt') + .3, state: 'walk', beats: 0, pc: enemy.column, pr: enemy.row, moveT: 0,
      hitT: .15, squashT: .36, wob: rand(state) * 6, face: offset });
    state.waveTotal++;
  }
}

// ---- time ----

function spawn(state: HordeState): boolean {
  const type = state.queue[0];
  const size = type === 'boss' ? 2 : 1;
  const taken = new Set(state.enemies.filter(enemy => enemy.state === 'gate')
    .flatMap(enemy => enemy.type === 'boss' ? [enemy.column, enemy.column + 1] : [enemy.column]));
  const options: number[] = [];
  for (let column = 1; column <= columnsOf(state) - size + 1; column++)
    if (Array.from({ length: size }, (_, d) => column + d).every(c => !taken.has(c))) options.push(column);
  if (!options.length) return false;
  state.queue.shift();
  const column = options[Math.floor(rand(state) * options.length)];
  state.enemies.push({ id: state.nextId++, type, column, row: rowsOf(state) + 1, hp: ENEMY[type].hp, max: ENEMY[type].hp,
    stepIn: stepSeconds(state, type), state: 'gate', beats: 0, weak: size === 2 ? 0 : undefined, weakT: 0, pc: column, pr: rowsOf(state) + 1,
    moveT: 1, hitT: 0, squashT: 0, wob: rand(state) * 6, face: 0,
    gateIn: size === 2 ? GATE_SECONDS.boss : GATE_SECONDS.single, gateBeats: size === 2 ? 4 : 2 });
  emit(state, { kind: 'spawn' });
  return true;
}

// Gate to arena. Returns false while the landing cells are taken.
function enterArena(state: HordeState, enemy: Enemy): boolean {
  const size = enemy.type === 'boss' ? 2 : 1;
  const row = rowsOf(state) - size + 1;
  if (!free(state, enemy.column, row, size, enemy)) return false;
  enemy.state = 'walk'; enemy.pc = enemy.column; enemy.pr = enemy.row; enemy.row = row; enemy.moveT = 0;
  enemy.stepIn = stepSeconds(state, enemy.type); enemy.beats = 0;
  return true;
}

function reachBase(state: HordeState, enemy: Enemy) {
  state.enemies = state.enemies.filter(item => item !== enemy);
  state.waveDone++;
  if (enemy.type === 'survivor') {
    state.saved++; state.hp = Math.min(100, state.hp + 5); state.score += 100;
    emit(state, { kind: 'saved', column: enemy.column, row: enemy.row });
    return;
  }
  const damage = ENEMY[enemy.type].dmg;
  emit(state, { kind: 'base', type: enemy.type, column: enemy.column, row: enemy.row, damage });
  breakCombo(state, 'BASE HIT');
  hurt(state, damage);
}

function stepEnemy(state: HordeState, enemy: Enemy): 'ok' | 'base' | 'end' {
  if (enemy.row - 1 < 1) { reachBase(state, enemy); return state.mode === 'play' ? 'base' : 'end'; }
  const size = enemy.type === 'boss' ? 2 : 1;
  let column = enemy.column;
  if (enemy.type === 'runner' && rand(state) < .3) {
    const side = rand(state) < .5 ? -1 : 1;
    if (free(state, enemy.column + side, enemy.row - 1, 1, enemy)) column = enemy.column + side;
  }
  const blocked = () => {
    if (state.playMode === 'beat') enemy.beats = BEATS[enemy.type] - 1;
    else enemy.stepIn = stepSeconds(state, enemy.type) * .45;
  };
  if (free(state, column, enemy.row - 1, size, enemy)) {
    enemy.pc = enemy.column; enemy.pr = enemy.row; enemy.face = column - enemy.column;
    enemy.column = column; enemy.row--; enemy.moveT = 0;
  } else if (size === 1) {
    const sides = [-1, 1].filter(side => free(state, enemy.column + side, enemy.row, 1, enemy));
    if (sides.length && rand(state) < .6) {
      const side = sides[Math.floor(rand(state) * sides.length)];
      enemy.pc = enemy.column; enemy.pr = enemy.row; enemy.column += side; enemy.moveT = 0; enemy.face = side;
    } else blocked();
  } else blocked();
  return 'ok';
}

function onBeat(state: HordeState, frozen: boolean): boolean {
  state.beatCount++;
  emit(state, { kind: 'beat', count: state.beatCount });
  if (state.queue.length && --state.spawnIn <= 0) {
    if (spawn(state)) {
      state.spawnIn = Math.max(1, Math.round(spawnGap(state) / beatLength(state) * between(state, .6, 1.3)));
      if (state.wave >= 3 && rand(state) < .22) state.spawnIn = 1;
    } else state.spawnIn = 1;
  }
  if (frozen) return true;
  for (const enemy of [...state.enemies]) {
    if (enemy.state === 'gate') {
      enemy.gateBeats = (enemy.gateBeats ?? 2) - 1;
      if (enemy.gateBeats <= 0 && !enterArena(state, enemy)) enemy.gateBeats = 1;
      continue;
    }
    enemy.beats = (enemy.beats ?? 0) + 1;
    if (enemy.beats < BEATS[enemy.type]) continue;
    enemy.beats = 0;
    if (stepEnemy(state, enemy) === 'end') return false;
  }
  return true;
}

export function tickHorde(state: HordeState, dt: number): void {
  if (!Number.isFinite(dt) || dt <= 0) return;
  state.time += dt; state.modeT += dt;
  state.hurtT = Math.max(0, state.hurtT - dt);
  for (const fx of state.cellFx) fx.t -= dt;
  state.cellFx = state.cellFx.filter(fx => fx.t > 0);
  if (state.mode === 'title' || state.mode === 'over') return;
  if (state.mode === 'inter') {
    if (state.modeT > 3) { state.mode = 'play'; state.modeT = 0; startWave(state, state.wave + 1); }
    return;
  }
  if (state.paused) return;

  for (const enemy of state.enemies) {
    enemy.hitT = Math.max(0, (enemy.hitT ?? 0) - dt);
    enemy.squashT = Math.max(0, (enemy.squashT ?? 0) - dt);
    enemy.moveT = Math.min(1, (enemy.moveT ?? 1) + dt * 7);
    if (enemy.type === 'boss' && (enemy.weakT = (enemy.weakT ?? 0) + dt) > 1.8) { enemy.weakT = 0; enemy.weak = ((enemy.weak ?? 0) + 1) % 4; }
  }
  if (state.comboIn > 0 && (state.comboIn -= dt) <= 0) breakCombo(state, 'COMBO TIMED OUT');
  const frozen = state.freezeIn > 0;
  state.freezeIn = Math.max(0, state.freezeIn - dt);

  if (state.playMode === 'beat') {
    state.beatClock += dt;
    for (let length = beatLength(state); state.beatClock >= length; length = beatLength(state)) {
      state.beatClock -= length;
      if (!onBeat(state, frozen) || state.mode !== 'play') return;
    }
  } else {
    if (state.queue.length && (state.spawnIn -= dt) <= 0) {
      if (spawn(state)) {
        state.spawnIn = spawnGap(state) * between(state, .6, 1.3);
        if (state.wave >= 3 && rand(state) < .22) state.spawnIn = .12;
      } else state.spawnIn = .2;
    }
    if (!frozen) for (const enemy of [...state.enemies]) {
      if (!state.enemies.includes(enemy)) continue;
      if (enemy.state === 'gate') {
        enemy.gateIn = (enemy.gateIn ?? GATE_SECONDS.single) - dt;
        if (enemy.gateIn <= 0 && !enterArena(state, enemy)) enemy.gateIn = .25;
        continue;
      }
      enemy.stepIn -= dt * state.speed * (1 + .07 * (state.wave - 1));
      if (enemy.stepIn > 0) continue;
      enemy.stepIn = stepSeconds(state, enemy.type);
      if (stepEnemy(state, enemy) === 'end') return;
    }
  }
  if (state.mode !== 'play') return;
  if (!state.queue.length && !state.enemies.length) {
    state.mode = 'inter'; state.modeT = 0;
    state.hp = Math.min(100, state.hp + 10);
    emit(state, { kind: 'clear', wave: state.wave });
  }
}

// 0 right after a step, 1 when the enemy is about to move. Free mode counts seconds, Beat mode counts beats.
export function stepProgress(state: HordeState, enemy: Enemy): number {
  if (enemy.state === 'gate') return 0;
  const progress = state.playMode === 'beat'
    ? ((enemy.beats ?? 0) + state.beatClock / beatLength(state)) / BEATS[enemy.type]
    : 1 - enemy.stepIn / stepSeconds(state, enemy.type);
  return Math.max(0, Math.min(1, progress));
}

// 1 when an enemy just surfaced, 0 when it is about to walk in.
export function gateFraction(state: HordeState, enemy: Enemy): number {
  const boss = enemy.type === 'boss';
  const fraction = state.playMode === 'beat'
    ? ((enemy.gateBeats ?? 2) - state.beatClock / beatLength(state)) / (boss ? 4 : 2)
    : (enemy.gateIn ?? GATE_SECONDS.single) / (boss ? GATE_SECONDS.boss : GATE_SECONDS.single);
  return Math.max(0, Math.min(1, fraction));
}

// ---- Launchpad lights ----

const scale = (rgb: Rgb, k: number): Rgb => rgb.map(v => Math.max(0, Math.min(63, Math.round(v * k)))) as unknown as Rgb;
const mix = (a: Rgb, b: Rgb, k: number): Rgb => a.map((v, i) => Math.max(0, Math.min(63, Math.round(v + (b[i] - v) * k)))) as unknown as Rgb;

export function ledFrame(state: HordeState, time = state.time, aids: Aids = 'medium'): Rgb[] {
  const frame = createLedFrame();
  // Pad coordinates: column 1-8 and row 1 (bottom) to 8. The side column and round top buttons are outside the 64-pad frame.
  const set = (column: number, row: number, rgb: Rgb) => {
    if (column >= 1 && column <= 8 && row >= 1 && row <= 8) frame[(8 - row) * 8 + column - 1] = rgb;
  };
  const toPad = (column: number, row: number) => state.layout === 'pad' ? { column, row }
    : column <= 8 ? { column, row: row + 4 } : null;
  if (state.mode === 'title') {
    for (let column = 1; column <= 8; column++) {
      const row = 8 - Math.floor((time * 3 + column * 1.7) % 10);
      if (row >= 1 && row <= 8) set(column, row, column % 3 === 0 ? ENEMY.runner.color : ENEMY.grunt.color);
      if (row + 1 >= 1 && row + 1 <= 8) set(column, row + 1, scale(ENEMY.grunt.color, .2));
    }
    return frame;
  }
  if (state.mode === 'over') {
    const k = .3 + .3 * Math.sin(time * 3);
    for (let row = 1; row <= 8; row++) for (let column = 1; column <= 8; column++) set(column, row, scale([63, 0, 0], k * (row <= 2 ? 1 : .4)));
    return frame;
  }
  const frozen = state.freezeIn > 0;
  const hpColor: Rgb = state.hp > 50 ? [0, 63, 10] : state.hp > 25 ? [63, 50, 0] : [63, 0, 0];
  if (state.layout === 'keys') for (let column = 1; column <= 8; column++) set(column, 4, scale(hpColor, .25));
  for (const enemy of state.enemies) {
    const color = ENEMY[enemy.type].color;
    if (enemy.state === 'gate') {
      for (let d = 0; d < (enemy.type === 'boss' ? 2 : 1); d++) {
        const pad = toPad(enemy.column + d, rowsOf(state));
        if (pad) set(pad.column, pad.row, scale(color, .12));
      }
      continue;
    }
    const progress = stepProgress(state, enemy);
    const size = enemy.type === 'boss' ? 2 : 1;
    if (!frozen && progress > .55 && enemy.row > 1 && aids !== 'low') for (let d = 0; d < size; d++) {
      const pad = toPad(enemy.column + d, enemy.row - 1);
      if (pad && !enemyAt(state, enemy.column + d, enemy.row - 1)) set(pad.column, pad.row, scale(color, .08 + .25 * (progress - .55)));
    }
    const near = 1 - (enemy.row - 1) / Math.max(1, rowsOf(state) - 1);
    const weak = enemy.type === 'boss' ? cellsOf(enemy)[enemy.weak ?? 0] : null;
    for (const [column, row] of cellsOf(enemy)) {
      const pad = toPad(column, row);
      if (!pad) continue;
      let rgb: Rgb;
      if (frozen) rgb = [20, 50, 63];
      else if (enemy.type === 'survivor') rgb = [40, 63, 63];
      else if (weak) rgb = weak[0] === column && weak[1] === row ? (Math.floor(time * 8) % 2 ? [63, 20, 40] : [63, 63, 63]) : [30, 0, 0];
      else {
        const pulse = near > .6 ? .65 + .35 * Math.sin(time * (2 + near * 10) * Math.PI) : 1;
        rgb = scale(color, (.45 + .55 * near) * pulse);
        if (enemy.row === 1 && Math.floor(time * 8) % 2) rgb = mix(rgb, [63, 63, 63], .5);
      }
      if ((enemy.hitT ?? 0) > 0) rgb = [63, 63, 63];
      set(pad.column, pad.row, rgb);
    }
  }
  for (const fx of state.cellFx) {
    const pad = toPad(fx.column, fx.row);
    if (!pad) continue;
    const color: Rgb = fx.kind === 'hit' ? [30, 50, 63] : fx.kind === 'bomb' ? [63, 24, 0] : fx.kind === 'armour' ? [30, 30, 30] : [63, 0, 0];
    if (!enemyAt(state, fx.column, fx.row) || fx.kind === 'bomb') set(pad.column, pad.row, scale(color, fx.t / fx.t0));
  }
  if (state.hurtT > 0 && state.layout === 'pad') for (let column = 1; column <= 8; column++)
    if (!enemyAt(state, column, 1)) set(column, 1, scale([63, 0, 0], state.hurtT * 2));
  if (state.mode === 'inter') {
    const sweep = Math.floor(state.modeT * 12) % 9;
    for (let row = 1; row <= 8; row++) set(sweep || 1, row, [0, 40, 50]);
  }
  // On the 8x8 arena the pad frame is all there is, so the base HP also shows on top-row pads that nothing else lights.
  if (state.layout === 'pad') for (let column = 1; column <= 8; column++) {
    const index = column - 1;
    if (column <= Math.round(state.hp / 100 * 8) && frame[index].every(channel => channel === 0))
      frame[index] = state.hp > 50 ? [0, 36, 8] : state.hp > 25 ? [40, 28, 0] : [48, 0, 0];
  }
  return frame;
}

export function noteCell(note: number): { column: number; row: number } {
  return { column: note % 10, row: Math.floor(note / 10) };
}

export function cellNote(column: number, row: number): number { return padNote(row, column); }
