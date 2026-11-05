(function () {
  'use strict';
  const R = SectorRules, g = R.create(), $ = id => document.getElementById(id);
  const canvas = $('arena'), ctx = canvas.getContext('2d');
  const embedded = parent !== window, reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const GRID = 752, CELL = GRID / 8, LEFT = 88, TOP = 80, SIZE = 900;
  let view = { scale: 1, x: 0, y: 0 }, input = null, output = null, audio = null;
  let sound = true, disposed = false, raf = 0, last = performance.now(), lastLed = 0;
  let ledCache = new Map(), effects = [], noticeUntil = 0, noticeText = '', keyboardAim = false;
  const mirrors = [], rgb = (c, a = 1) => `rgba(${c.map(v => Math.round(v * 255 / 63)).join(',')},${a})`;
  const point = (x, y) => [LEFT + x * CELL, TOP + y * CELL];
  const noise = n => { const v = Math.sin(n * 127.1 + 311.7) * 43758.5453; return v - Math.floor(v); };

  function wakeAudio() {
    if (!sound) return;
    try { audio ??= new AudioContext(); if (audio.state === 'suspended') void audio.resume().catch(() => {}); } catch { /* Gameplay also works without audio. */ }
  }
  function tone(kind) {
    if (!sound || !audio || audio.state !== 'running') return;
    const sounds = { shot: [490, 140, .07], hit: [180, 90, .08], kill: [320, 55, .18], repair: [450, 900, .25], damage: [100, 25, .3], pulse: [1000, 40, .6], wave: [240, 480, .2], over: [180, 30, .8] };
    const s = sounds[kind]; if (!s) return;
    const osc = audio.createOscillator(), gain = audio.createGain(), t = audio.currentTime;
    osc.type = kind === 'damage' || kind === 'kill' ? 'sawtooth' : 'sine';
    osc.frequency.setValueAtTime(s[0], t); osc.frequency.exponentialRampToValueAtTime(s[1], t + s[2]);
    gain.gain.setValueAtTime(.0001, t); gain.gain.exponentialRampToValueAtTime(.055, t + .008); gain.gain.exponentialRampToValueAtTime(.0001, t + s[2]);
    osc.connect(gain); gain.connect(audio.destination); osc.start(t); osc.stop(t + s[2]);
    osc.onended = () => { osc.disconnect(); gain.disconnect(); };
  }
  function announce(text, seconds = 2) { noticeText = text; noticeUntil = performance.now() + seconds * 1000; }
  function start() {
    wakeAudio(); R.start(g); effects = []; keyboardAim = false; $('end').hidden = true; canvas.focus({ preventScroll: true });
  }
  function shoot(x, y) { wakeAudio(); R.hit(g, x, y); }
  function pulse() { wakeAudio(); R.firePulse(g); }
  function togglePause() {
    R.pause(g);
    if (!g.paused) canvas.focus({ preventScroll: true });
  }
  function disconnected() {
    if (g.mode === 'play' && gameMenu.inputMode === 'hardware') {
      g.paused = true; gameMenu.open(); announce('CONTROLLER DISCONNECTED', 4);
    }
  }
  function midi(data) {
    if (disposed || !Array.isArray(data) && !(data instanceof Uint8Array)) return;
    if (data.length < 3 || !Array.from(data).every(n => Number.isInteger(n) && n >= 0 && n <= 255)) return;
    const [status, note, velocity] = data, command = status & 0xf0;
    if (velocity <= 0 || velocity > 127 || note > 127 || command !== 0x90 && command !== 0xb0) return;
    if (gameMenu.inputMode !== 'hardware' || !gameMenu.checkHardwareConnected()) return;
    const session = command === 0xb0 && note === 108;
    if (gameMenu.isOpen) { if (session) gameMenu.primaryAction(); return; }
    if (session) {
      if (g.mode === 'over') { if (g.overTime > .8) start(); }
      else togglePause();
      return;
    }
    if (command !== 0x90) return;
    if (note === 89) { pulse(); return; }
    if (note === 79) { togglePause(); return; }
    const cell = R.noteCell(note);
    if (cell) shoot(cell.x, cell.y);
  }
  function deviceChange(devices) {
    if (input) input.onmidimessage = null;
    const device = devices.get('lp');
    const nextOutput = device?.output ?? null, changed = output !== nextOutput;
    input = device?.input ?? null; output = device?.output ?? null;
    if (input) input.onmidimessage = event => midi(event.data);
    if (changed && output && gameMenu.midiAccess?.sysexEnabled) {
      // Session layout gives the decimal grid notes, even after another app used User mode.
      try { output.send([240, 0, 32, 41, 2, 24, 34, 0, 247]); }
      catch { gameMenu.toast('Could not set the Launchpad layout. Reconnect the controller.', 'warn'); disconnected(); }
    }
    ledCache.clear();
    if (!device?.connected) disconnected();
  }
  window.gameMenu = new LaunchBandMenu({
    gameId: 'sector', gameTitle: 'Sector Defense', gameSubtitle: 'Defend the station. Every lit pad is a target.',
    shortTitle: 'Sector Defense', kicker: 'Launchpad MKII · 8×8', skin: 'sector', emblem: '', autoParams: false,
    requiredDevices: [{ id: 'lp', name: 'Launchpad MKII', match: /launchpad\s*(mk\s*(ii|2))\b/i, required: true }],
    hud: { adopt: ['#connection'] },
    parameters: [],
    keybindings: [
      { key: 'Pad / Click', desc: 'Shoot the target in that cell. Green targets repair your shield.' },
      { key: 'Volume / Space', desc: 'Fire a full-charge pulse. Each kill adds 10% charge.' },
      { key: 'Session', desc: 'Start, pause, resume, or restart after station loss.' },
      { key: 'Pan / P', desc: 'Pause or resume.' },
      { key: 'Arrows + Enter', desc: 'Move the aiming cell and fire.' },
      { key: 'Esc', desc: 'Open the menu and pause.' }
    ],
    onStart: start, onRestart: start, onPause: () => { if (g.mode === 'play') g.paused = true; },
    onResume: () => {
      if (gameMenu.inputMode === 'hardware' && !gameMenu.checkHardwareConnected()) { gameMenu.open(); return; }
      if (g.mode === 'over') start(); else { wakeAudio(); g.paused = false; canvas.focus({ preventScroll: true }); }
    },
    onDeviceChange: deviceChange
  });
  function message(event) {
    if (!embedded || event.source !== parent || !event.data || typeof event.data !== 'object') return;
    if (event.data.lpd === 'midi') midi(event.data.data);
    if (event.data.lpd === 'status') { ledCache.clear(); if (event.data.state !== 'connected') disconnected(); }
  }
  addEventListener('message', message);

  function sendLeds(frame, force = false) {
    if (gameMenu.isTestingLeds && !force) { ledCache.clear(); return; }
    const changed = frame.filter(([n, ...c]) => force || ledCache.get(n) !== c.join(','));
    if (!changed.length) return;
    try {
      if (embedded) parent.postMessage({ lpd: 'leds', leds: changed }, '*');
      else if (output?.state === 'connected') {
        if (gameMenu.midiAccess?.sysexEnabled) output.send([240, 0, 32, 41, 2, 24, 11, ...changed.flat(), 247]);
        else for (const [n, r, green, b] of changed) {
          const palette = !r && !green && !b ? 0 : green > r && green > b ? 21 : b > green ? (r > 20 ? 53 : 45) : 13;
          output.send([n >= 104 ? 0xb0 : 0x90, n, palette]);
        }
      }
      for (const [n, ...c] of changed) ledCache.set(n, c.join(','));
    } catch { ledCache.clear(); disconnected(); }
  }

  for (let y = 0; y < 8; y++) for (let x = 0; x < 8; x++) {
    const button = document.createElement('button');
    button.type = 'button'; button.tabIndex = -1;
    button.setAttribute('aria-label', `Fire at ${String.fromCharCode(65 + x)}${y + 1}`);
    button.onclick = () => { if (!gameMenu.isOpen) shoot(x, y); };
    $('mirror').appendChild(button); mirrors.push(button);
  }
  $('pulse').onclick = pulse; $('pause').onclick = togglePause; $('retry').onclick = start;
  $('sound').onclick = () => { sound = !sound; $('sound').textContent = sound ? 'Sound on' : 'Sound off'; $('sound').setAttribute('aria-pressed', String(sound)); wakeAudio(); };
  canvas.addEventListener('pointerdown', event => {
    if (event.button !== 0 || gameMenu.isOpen) return;
    canvas.focus({ preventScroll: true }); keyboardAim = false;
    const rect = canvas.getBoundingClientRect(), x = ((event.clientX - rect.left - view.x) / view.scale - LEFT) / CELL;
    const y = ((event.clientY - rect.top - view.y) / view.scale - TOP) / CELL;
    if (x >= 0 && x < 8 && y >= 0 && y < 8) shoot(Math.floor(x), Math.floor(y));
  });
  addEventListener('keydown', event => {
    if (gameMenu.isOpen || event.defaultPrevented || event.ctrlKey || event.altKey || event.metaKey || event.target.closest?.('button,input,select,textarea,a')) return;
    const directions = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
    const d = directions[event.code];
    if (d) { event.preventDefault(); keyboardAim = true; g.aim.x = Math.max(0, Math.min(7, g.aim.x + d[0])); g.aim.y = Math.max(0, Math.min(7, g.aim.y + d[1])); }
    else if (['Space', 'Enter', 'KeyP'].includes(event.code)) {
      event.preventDefault(); if (event.repeat) return;
      if (event.code === 'Space') pulse();
      else if (event.code === 'KeyP') togglePause();
      else if (g.mode === 'over') start();
      else shoot(g.aim.x, g.aim.y);
    }
  });
  document.addEventListener('visibilitychange', () => { if (document.hidden && g.mode === 'play') g.paused = true; last = performance.now(); });
  addEventListener('blur', () => { if (g.mode === 'play') g.paused = true; });

  const background = document.createElement('canvas'); background.width = background.height = SIZE;
  const bg = background.getContext('2d');
  bg.fillStyle = '#010603'; bg.fillRect(0, 0, SIZE, SIZE);
  const cloud = bg.createRadialGradient(690, 270, 0, 690, 270, 280);
  cloud.addColorStop(0, '#142a2328'); cloud.addColorStop(1, '#00000000'); bg.fillStyle = cloud; bg.fillRect(0, 0, SIZE, SIZE);
  for (let i = 0; i < 180; i++) {
    const x = noise(i) * SIZE, y = noise(i + 400) * SIZE, s = i % 11 ? 1 : 2;
    bg.fillStyle = `rgba(177,204,171,${.08 + noise(i + 700) * .35})`; bg.fillRect(x, y, s, s);
  }
  bg.strokeStyle = '#6d92722b'; bg.lineWidth = 1;
  for (let i = 0; i <= 8; i++) {
    bg.beginPath(); bg.moveTo(LEFT + i * CELL, TOP); bg.lineTo(LEFT + i * CELL, TOP + GRID); bg.stroke();
    bg.beginPath(); bg.moveTo(LEFT, TOP + i * CELL); bg.lineTo(LEFT + GRID, TOP + i * CELL); bg.stroke();
  }
  function polygon(points) {
    ctx.beginPath(); points.forEach(([x, y], i) => i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)); ctx.closePath();
  }
  function drawTarget(e) {
    const [x, y] = point(e.x, e.y), color = R.COLORS[e.kind], r = e.kind === 'rock' ? 23 : e.kind === 'armored' ? 22 : 17;
    const cellX = Math.floor(e.x), cellY = Math.floor(e.y), [cx, cy] = point(cellX, cellY);
    ctx.strokeStyle = rgb(color, Math.hypot(e.x - 4, e.y - 4) < 1.7 ? .28 : .09); ctx.lineWidth = 1;
    ctx.strokeRect(cx + 2, cy + 2, CELL - 4, CELL - 4);
    ctx.save(); ctx.translate(x, y); ctx.rotate(e.angle + (reduced ? 0 : g.time * (e.kind === 'rock' ? .17 : .07)));
    if (e.kind === 'rock') {
      const points = Array.from({ length: 11 }, (_, i) => {
        const a = i / 11 * Math.PI * 2, radius = r * (.72 + noise(e.id * 19 + i) * .45);
        return [Math.cos(a) * radius, Math.sin(a) * radius];
      });
      const grad = ctx.createLinearGradient(-r, -r, r, r); grad.addColorStop(0, '#c5c3a0'); grad.addColorStop(.5, '#777c67'); grad.addColorStop(1, '#1d2c25');
      polygon(points); ctx.fillStyle = grad; ctx.fill(); ctx.strokeStyle = '#b6b99355'; ctx.stroke(); ctx.clip();
      for (let i = 0; i < 24; i++) {
        const px = (noise(e.id + i * 7) - .5) * r * 2, py = (noise(e.id + i * 13) - .5) * r * 2;
        ctx.fillStyle = i % 3 ? '#07150f38' : '#e2dec827'; ctx.beginPath(); ctx.ellipse(px, py, 1 + noise(i) * 4, 1 + noise(i + 1) * 3, 0, 0, Math.PI * 2); ctx.fill();
      }
    } else {
      ctx.shadowColor = rgb(color, .4); ctx.shadowBlur = 12;
      const pts = e.kind === 'armored' ? [[-r, -r * .6], [r * .4, -r], [r, r * .5], [-r * .4, r]] : [[0, -r * 1.4], [r, 0], [0, r], [-r, 0]];
      polygon(pts); ctx.fillStyle = rgb(color, .67); ctx.fill(); ctx.shadowBlur = 0; ctx.strokeStyle = rgb(color, .7); ctx.stroke();
      polygon([pts[0], pts[1], [0, 0]]); ctx.fillStyle = '#e0fff02b'; ctx.fill();
      polygon([pts[2], pts[3], [0, 0]]); ctx.fillStyle = '#020a2444'; ctx.fill();
      ctx.beginPath(); ctx.moveTo(...pts[0]); ctx.lineTo(...pts[2]); ctx.moveTo(...pts[1]); ctx.lineTo(...pts[3]); ctx.strokeStyle = '#d2f6ff35'; ctx.stroke();
      if (e.kind === 'repair') { ctx.fillStyle = '#b4ffc0'; ctx.fillRect(-6, -1.5, 12, 3); ctx.fillRect(-1.5, -6, 3, 12); }
    }
    ctx.restore();
    if (e.hp > 1) { ctx.font = '14px Consolas, monospace'; ctx.fillStyle = '#c8d99d'; ctx.textAlign = 'center'; ctx.fillText(String(e.hp), x, y - r - 10); }
  }
  function station() {
    const [x, y] = point(4, 4);
    ctx.save(); ctx.translate(x, y);
    const glow = ctx.createRadialGradient(0, 0, 12, 0, 0, 77); glow.addColorStop(0, '#8bd68325'); glow.addColorStop(1, '#00ff4100'); ctx.fillStyle = glow; ctx.fillRect(-77, -77, 154, 154);
    polygon(Array.from({ length: 8 }, (_, i) => { const a = (i + .5) * Math.PI / 4; return [Math.cos(a) * 34, Math.sin(a) * 34]; }));
    const metal = ctx.createLinearGradient(-35, -35, 35, 35); metal.addColorStop(0, '#7a9480'); metal.addColorStop(.45, '#3f5d4e'); metal.addColorStop(1, '#152820');
    ctx.fillStyle = metal; ctx.fill(); ctx.strokeStyle = '#abc2a8'; ctx.lineWidth = 2.5; ctx.stroke();
    ctx.beginPath(); ctx.arc(0, 0, 28, 0, Math.PI * 2); ctx.fillStyle = '#203e30'; ctx.fill(); ctx.strokeStyle = '#637f6b'; ctx.stroke();
    for (let i = 0; i < 8; i++) { const a = (i + .5) * Math.PI / 4; ctx.fillStyle = '#bcc7a0'; ctx.beginPath(); ctx.arc(Math.cos(a) * 30, Math.sin(a) * 30, 1.4, 0, Math.PI * 2); ctx.fill(); }
    ctx.save(); ctx.rotate(Math.atan2(g.aim.y + .5 - 4, g.aim.x + .5 - 4));
    ctx.fillStyle = '#1a2e23'; ctx.fillRect(15, -7, 39, 14); ctx.fillStyle = '#8ca68a'; ctx.fillRect(13, -4, 40, 8); ctx.fillStyle = '#475e4b'; ctx.fillRect(22, 0, 32, 4); ctx.fillStyle = '#b3d57f'; ctx.fillRect(48, -4, 4, 8); ctx.restore();
    ctx.beginPath(); ctx.arc(0, 0, 16, 0, Math.PI * 2); ctx.fillStyle = '#476e4b'; ctx.fill(); ctx.strokeStyle = '#a9c797'; ctx.lineWidth = 1.5; ctx.stroke();
    ctx.shadowColor = g.flash > 0 ? '#ffb000' : '#aeff66'; ctx.shadowBlur = 18; ctx.fillStyle = g.flash > 0 ? '#ffb000' : '#b6ed71'; ctx.beginPath(); ctx.arc(0, 0, 5, 0, Math.PI * 2); ctx.fill(); ctx.restore();
  }
  function draw() {
    const dpr = Math.min(2, devicePixelRatio || 1), w = canvas.clientWidth, h = canvas.clientHeight;
    if (!w || !h) return;
    if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) { canvas.width = Math.round(w * dpr); canvas.height = Math.round(h * dpr); }
    const s = Math.min(w, h) / SIZE; view = { scale: s, x: (w - SIZE * s) / 2, y: (h - SIZE * s) / 2 };
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0); ctx.fillStyle = '#010603'; ctx.fillRect(0, 0, w, h);
    ctx.translate(view.x, view.y); ctx.scale(s, s); ctx.drawImage(background, 0, 0);
    const demo = [
      { id: 1, kind: 'scout', x: 1.2, y: 1.1, hp: 1, angle: .2 }, { id: 2, kind: 'rock', x: 3.8, y: .8, hp: 2, angle: 1 },
      { id: 3, kind: 'armored', x: 6.6, y: 2.7, hp: 3, angle: .3 }, { id: 4, kind: 'repair', x: 2.1, y: 6.3, hp: 1, angle: .1 },
      { id: 5, kind: 'rock', x: .8, y: 4.2, hp: 2, angle: 2 }
    ];
    const enemies = g.mode === 'title' ? demo : g.enemies;
    ctx.textAlign = 'center'; ctx.textBaseline = 'middle'; ctx.font = '26px Consolas, monospace';
    for (let i = 0; i < 8; i++) {
      const col = enemies.find(e => Math.floor(e.x) === i), row = enemies.find(e => Math.floor(e.y) === i);
      ctx.shadowBlur = col ? 9 : 0; ctx.shadowColor = col ? rgb(R.COLORS[col.kind]) : 'transparent'; ctx.fillStyle = col ? rgb(R.COLORS[col.kind]) : '#6c856d'; ctx.fillText(String.fromCharCode(65 + i), LEFT + (i + .5) * CELL, 45);
      ctx.shadowBlur = row ? 9 : 0; ctx.shadowColor = row ? rgb(R.COLORS[row.kind]) : 'transparent'; ctx.fillStyle = row ? rgb(R.COLORS[row.kind]) : '#6c856d'; ctx.fillText(String(i + 1), 48, TOP + (i + .5) * CELL);
    }
    ctx.shadowBlur = 0;
    if (keyboardAim && g.mode === 'play') { const [x, y] = point(g.aim.x, g.aim.y); ctx.strokeStyle = '#ffb000'; ctx.lineWidth = 2; ctx.strokeRect(x + 5, y + 5, CELL - 10, CELL - 10); }
    for (const e of enemies) drawTarget(e);
    for (const e of effects) {
      const [x, y] = point(e.x, e.y), a = Math.max(0, 1 - e.age / e.life);
      if (e.kind === 'shot') {
        ctx.strokeStyle = e.hit ? `rgba(189,255,142,${a})` : `rgba(119,155,117,${a * .5})`; ctx.lineWidth = e.hit ? 2 : 1;
        ctx.beginPath(); ctx.moveTo(...point(4, 4)); ctx.lineTo(x, y); ctx.stroke();
      } else {
        ctx.strokeStyle = rgb(e.color, a); ctx.lineWidth = 1.5;
        for (let i = 0; i < (reduced ? 4 : 9); i++) {
          const angle = i * Math.PI * 2 / 9, distance = 5 + e.age * 65;
          ctx.beginPath(); ctx.moveTo(x + Math.cos(angle) * distance, y + Math.sin(angle) * distance); ctx.lineTo(x + Math.cos(angle) * (distance + 5), y + Math.sin(angle) * (distance + 5)); ctx.stroke();
        }
      }
    }
    if (g.pulse > 0 && !reduced) { ctx.strokeStyle = `rgba(147,255,167,${g.pulse})`; ctx.lineWidth = 2; ctx.beginPath(); ctx.arc(...point(4, 4), (1 - g.pulse) * 620 + 40, 0, Math.PI * 2); ctx.stroke(); }
    station();
  }
  function ui(frame) {
    $('shield').innerHTML = `${g.hp}<span>%</span>`; $('shieldMeter').value = g.hp;
    $('wave').textContent = String(g.wave || 1).padStart(2, '0'); $('score').textContent = String(g.score).padStart(6, '0');
    $('combo').textContent = `${g.combo} CHAIN`; $('multiplier').textContent = `×${Math.min(4, 1 + Math.floor(g.combo / 5))}`;
    $('charge').textContent = g.charge >= 100 ? 'READY' : `${g.charge}%`; $('chargeMeter').value = g.charge;
    $('pulse').disabled = g.charge < 100 || g.paused || g.mode !== 'play';
    $('pause').disabled = g.mode !== 'play'; $('pause').textContent = g.paused ? 'Resume' : 'Pause'; $('pause').setAttribute('aria-pressed', String(g.paused));
    $('state').textContent = g.mode === 'title' ? 'STANDBY' : g.mode === 'over' ? 'OFFLINE' : g.paused ? 'PAUSED' : 'ACTIVE';
    $('connection').textContent = gameMenu.inputMode === 'keyboard' ? 'Keyboard & mouse' : gameMenu.checkHardwareConnected() ? 'Launchpad MKII connected' : 'Launchpad disconnected';
    $('notice').textContent = g.paused && !gameMenu.isOpen ? 'PAUSED · SESSION / P TO RESUME' : performance.now() < noticeUntil ? noticeText : '';
    for (const [note, ...color] of frame) {
      const cell = R.noteCell(note); if (!cell) continue;
      const button = mirrors[cell.y * 8 + cell.x], lit = color.some(c => c > 0);
      button.style.background = lit ? rgb(color) : '#0b170f'; button.style.boxShadow = lit ? `0 0 7px ${rgb(color, .18)}` : 'none';
    }
  }
  function loop(now) {
    if (disposed) return;
    const dt = Math.min(.05, (now - last) / 1000); last = now;
    R.tick(g, dt);
    for (const e of g.events.splice(0)) {
      tone(e.kind);
      if (e.kind === 'wave') announce(`WAVE ${String(e.wave).padStart(2, '0')}`);
      if (e.kind === 'clear') announce('SECTOR CLEAR');
      if (e.kind === 'repair') announce('+15 SHIELD', 1);
      if (e.x !== undefined) effects.push({ ...e, age: 0, life: e.kind === 'shot' ? .16 : .5 });
      if (e.kind === 'over') {
        $('end').hidden = false; $('result').textContent = `Wave ${g.wave} · ${g.score.toLocaleString()} points · ${g.kills} threats destroyed`;
        $('retry').focus({ preventScroll: true });
      }
    }
    if (!g.paused) for (const e of effects) e.age += dt;
    effects = effects.filter(e => e.age < e.life);
    draw();
    if (now - lastLed >= 50) { const frame = R.leds(g); sendLeds(frame); ui(frame); lastLed = now; }
    raf = requestAnimationFrame(loop);
  }
  raf = requestAnimationFrame(loop);
  addEventListener('pagehide', () => {
    disposed = true; cancelAnimationFrame(raf); sendLeds(R.leds(g).map(([n]) => [n, 0, 0, 0]), true);
    if (input) input.onmidimessage = null; removeEventListener('message', message);
    if (audio) void audio.close().catch(() => {});
  });
})();
