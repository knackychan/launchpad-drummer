/* Pure rules shared by the canvas game and Node checks; classic script also works offline. */
(function (global) {
  'use strict';
  const COLORS = { scout: [10, 39, 63], rock: [46, 37, 20], armored: [39, 12, 63], repair: [0, 63, 16] };
  const TYPES = {
    scout: { hp: 1, speed: .38, points: 100, damage: 12 },
    rock: { hp: 2, speed: .24, points: 180, damage: 18 },
    armored: { hp: 3, speed: .3, points: 300, damage: 24 },
    repair: { hp: 1, speed: .23, points: 50, damage: 0 }
  };
  const cellNote = (x, y) => (8 - y) * 10 + x + 1;
  function noteCell(note) {
    if (!Number.isInteger(note)) return null;
    const x = note % 10 - 1, y = 8 - Math.floor(note / 10);
    return x >= 0 && x < 8 && y >= 0 && y < 8 ? { x, y } : null;
  }
  function create(seed = 137) {
    return { seed: seed >>> 0 || 1, mode: 'title', paused: false, time: 0, wave: 0,
      hp: 100, score: 0, combo: 0, charge: 0, kills: 0, shots: 0, hits: 0,
      enemies: [], events: [], nextId: 1, remaining: 0, spawnIn: 0, rest: 0, cooldown: 0,
      aim: { x: 4, y: 1 }, flash: 0, pulse: 0, overTime: 0 };
  }
  function random(g) {
    g.seed = (Math.imul(g.seed, 1664525) + 1013904223) >>> 0;
    return g.seed / 4294967296;
  }
  function wave(g) {
    g.wave++;
    g.remaining = 6 + g.wave * 2;
    g.spawnIn = .3;
    g.events.push({ kind: 'wave', wave: g.wave });
  }
  function start(g) {
    Object.assign(g, create(g.seed));
    g.mode = 'play';
    wave(g);
  }
  function spawn(g) {
    const roll = random(g), kind = roll < .1 ? 'repair' : roll < .48 ? 'scout' : roll < .86 || g.wave < 2 ? 'rock' : 'armored';
    const edge = Math.floor(random(g) * 4), along = .35 + random(g) * 7.3;
    const x = edge === 0 ? .25 : edge === 1 ? 7.75 : along;
    const y = edge === 2 ? .25 : edge === 3 ? 7.75 : along;
    const type = TYPES[kind];
    g.enemies.push({ id: g.nextId++, kind, x, y, hp: type.hp,
      speed: type.speed * (1 + Math.min(12, g.wave - 1) * .08), angle: random(g) * Math.PI * 2 });
  }
  function hit(g, x, y) {
    if (g.mode !== 'play' || g.paused || g.cooldown > 0 || !Number.isInteger(x) || !Number.isInteger(y) || x < 0 || y < 0 || x > 7 || y > 7) return false;
    g.cooldown = .12;
    g.aim = { x, y };
    g.shots++;
    // Shoot the closest threat first when two targets share a pad.
    const targets = g.enemies.filter(e => Math.floor(e.x) === x && Math.floor(e.y) === y)
      .sort((a, b) => Number(a.kind === 'repair') - Number(b.kind === 'repair') || Math.hypot(a.x - 4, a.y - 4) - Math.hypot(b.x - 4, b.y - 4));
    const target = targets[0];
    g.events.push({ kind: 'shot', x: target?.x ?? x + .5, y: target?.y ?? y + .5, hit: !!target });
    if (!target) { g.combo = 0; return false; }
    g.hits++;
    target.hp--;
    if (target.hp > 0) { g.events.push({ kind: 'hit', x: target.x, y: target.y, color: COLORS[target.kind] }); return true; }
    g.enemies.splice(g.enemies.indexOf(target), 1);
    if (target.kind === 'repair') {
      g.hp = Math.min(100, g.hp + 15);
      g.score += TYPES.repair.points;
    } else {
      g.combo++;
      g.kills++;
      g.charge = Math.min(100, g.charge + 10);
      g.score += TYPES[target.kind].points * Math.min(4, 1 + Math.floor(g.combo / 5));
    }
    g.events.push({ kind: target.kind === 'repair' ? 'repair' : 'kill', x: target.x, y: target.y, color: COLORS[target.kind] });
    return true;
  }
  function firePulse(g) {
    if (g.mode !== 'play' || g.paused || g.charge < 100) return false;
    g.charge = 0;
    g.pulse = 1;
    for (const e of g.enemies) if (e.kind !== 'repair') {
      g.score += TYPES[e.kind].points;
      g.kills++;
      g.events.push({ kind: 'kill', x: e.x, y: e.y, color: COLORS[e.kind] });
    }
    g.enemies = g.enemies.filter(e => e.kind === 'repair');
    g.events.push({ kind: 'pulse' });
    return true;
  }
  function pause(g) { if (g.mode === 'play') g.paused = !g.paused; }
  function tick(g, dt) {
    if (!Number.isFinite(dt) || dt <= 0 || g.paused) return;
    dt = Math.min(dt, .05); // Do not fast-forward into a loss after a suspended frame.
    if (g.mode === 'over') { g.overTime += dt; return; }
    if (g.mode !== 'play') return;
    g.time += dt;
    g.cooldown = Math.max(0, g.cooldown - dt);
    g.flash = Math.max(0, g.flash - dt * 2);
    g.pulse = Math.max(0, g.pulse - dt);
    if (g.remaining > 0) {
      g.spawnIn -= dt;
      if (g.spawnIn <= 0) { spawn(g); g.remaining--; g.spawnIn = Math.max(.7, 1.7 - g.wave * .07); }
    }
    for (let i = g.enemies.length - 1; i >= 0; i--) {
      const e = g.enemies[i], dx = 4 - e.x, dy = 4 - e.y, distance = Math.hypot(dx, dy);
      if (distance <= .55) {
        g.enemies.splice(i, 1);
        if (e.kind !== 'repair') {
          g.hp = Math.max(0, g.hp - TYPES[e.kind].damage);
          g.combo = 0;
          g.flash = 1;
          g.events.push({ kind: 'damage' });
          if (g.hp === 0) { g.mode = 'over'; g.events.push({ kind: 'over' }); break; }
        }
      } else { e.x += dx / distance * e.speed * dt; e.y += dy / distance * e.speed * dt; }
    }
    if (g.mode === 'play' && !g.remaining && !g.enemies.length) {
      if (g.rest === 0) { g.rest = 2.5; g.events.push({ kind: 'clear' }); }
      else { g.rest -= dt; if (g.rest <= 0) { g.rest = 0; wave(g); } }
    }
  }
  function leds(g) {
    const frame = new Map();
    for (let y = 0; y < 8; y++) for (let x = 0; x < 8; x++) frame.set(cellNote(x, y), [0, 0, 0]);
    for (let y = 1; y <= 8; y++) frame.set(y * 10 + 9, [0, 0, 0]);
    for (let n = 104; n <= 111; n++) frame.set(n, [0, 0, 0]);
    const green = g.mode === 'over' ? [32, 0, 0] : [0, 9, 2];
    for (const x of [3, 4]) for (const y of [3, 4]) frame.set(cellNote(x, y), green);
    for (const e of g.enemies) {
      const n = cellNote(Math.floor(e.x), Math.floor(e.y));
      if (frame.has(n)) frame.set(n, COLORS[e.kind]);
    }
    if (g.mode === 'over') {
      for (let i = 0; i < 8; i++) { frame.set(cellNote(i, i), [40, 0, 0]); frame.set(cellNote(7 - i, i), [40, 0, 0]); }
    }
    frame.set(89, g.charge >= 100 ? [0, 63, 16] : [Math.round(g.charge / 5), Math.round(g.charge / 8), 0]);
    frame.set(79, g.paused ? [63, 30, 0] : [6, 8, 3]);
    frame.set(108, g.paused || g.mode !== 'play' ? [0, 40, 10] : [12, 12, 4]);
    return [...frame].map(([n, rgb]) => [n, ...rgb]);
  }
  global.SectorRules = { COLORS, TYPES, cellNote, noteCell, create, start, hit, firePulse, pause, tick, leds };
})(globalThis);
