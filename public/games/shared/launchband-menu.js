/**
 * =============================================================================
 * LAUNCHBAND SHARED MENU & HARDWARE MANAGER
 * One menu for every LaunchBand game: same structure and controls everywhere,
 * dressed in each game's own look (skin), plus an in-game HUD strip that
 * replaces the old settings header.
 * =============================================================================
 */

(function (global) {
  'use strict';

  // Game Registry
  const ALL_GAMES = [
    {
      id: 'broadside',
      title: 'Broadside: Naval Warfare',
      sub: 'Asymmetric 2-Player Co-op Flotilla Combat',
      requiredHardware: ['Launch Control XL (Gunner)', 'Launchpad MK2 (Damage Control)'],
      path: '../broadside/index.html',
      hostPath: '/games/broadside/index.html',
      hidden: true // Hidden for now: incomplete prototype
    },
    {
      id: 'horde',
      title: 'Horde: Hold the Beach',
      sub: 'Rhythm Beach Defense & Score Attack',
      requiredHardware: ['Launchpad MK2 (8×8 Grid)'],
      path: '../horde/index.html',
      hostPath: '/games/horde/index.html'
    },
    {
      id: 'cockpit',
      title: 'Cockpit 3D: Space Sim',
      sub: 'Aggregated Ship Equipment & Tactical Dogfight',
      requiredHardware: ['Launchpad MK2 (9×9 Matrix)'],
      path: '../cockpit/index.html',
      hostPath: '/games/cockpit/index.html'
    },
    {
      id: 'chess',
      title: 'Launchpad Chess',
      sub: 'Tactical 8×8 Chess with RGB Piece Illumination',
      requiredHardware: ['Launchpad MK2 (8×8 Board)'],
      path: '../chess/index.html',
      hostPath: '/games/chess/index.html'
    },
    {
      id: 'xiangqi',
      title: 'Xiangqi: Chinese Chess',
      sub: 'Classic Xiangqi Tactical Warfare',
      requiredHardware: ['Launchpad MK2 (Pads & Nav)'],
      path: '../xiangqi/index.html',
      hostPath: '/games/xiangqi/index.html'
    }
  ];

  // Per-game look and HUD wiring. A game config can override any of these.
  // hud.adopt: elements of the old header kept visible in the HUD strip (moved, so listeners survive).
  // escPassthrough: return true when the game itself needs Esc (e.g. dropping a held piece).
  const pieceHeld = () => Boolean(global.__G && global.__G.sel >= 0);
  const GAME_LOOKS = {
    chess: {
      skin: 'study', emblem: '♞︎', shortTitle: 'Chess', kicker: 'Lamp-lit table',
      playLabel: 'Play', restartLabel: 'New game',
      hud: { adopt: ['#aiStatus', '#undoBtn', '#newBtn', '#midi', '#mirror'] },
      escPassthrough: pieceHeld
    },
    xiangqi: {
      skin: 'lacquer', emblem: '帥', shortTitle: 'Xiangqi 象棋', kicker: 'River & palace',
      playLabel: 'Play', restartLabel: 'New game',
      hud: { adopt: ['#aiStatus', '#undoBtn', '#newBtn', '#midi', '#mirror'] },
      escPassthrough: pieceHeld
    },
    horde: {
      skin: 'beach', emblem: '✶', shortTitle: 'Hold the Beach', kicker: 'Horde defense',
      playLabel: 'Defend', restartLabel: 'Restart waves',
      hud: { adopt: ['#midi', '#mirror'] }
    },
    cockpit: {
      skin: 'hud', emblem: '✦', shortTitle: 'Cockpit 3D', kicker: 'Flight deck',
      playLabel: 'Launch', restartLabel: 'New sortie',
      hud: { adopt: ['#midi', '#mirror'] }
    },
    broadside: {
      skin: 'naval', emblem: '⚓', shortTitle: 'Broadside', kicker: 'Co-op flotilla',
      playLabel: 'Set sail', restartLabel: 'New battle',
      hud: { adopt: ['#fullSalvoBtn', '#pauseBtn', '#midi-box'] },
      autoParams: false // header selects already map to boatCount / battleMode
    }
  };

  const ICONS = {
    play: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 4.5v15l12-7.5z" fill="currentColor"/></svg>',
    restart: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 12a8 8 0 1 0 2.4-5.7M4 4v4.5h4.5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></svg>',
    settings: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h10M18 7h2M4 17h2M10 17h10" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/><circle cx="16" cy="7" r="2.2" fill="none" stroke="currentColor" stroke-width="2.2"/><circle cx="8" cy="17" r="2.2" fill="none" stroke="currentColor" stroke-width="2.2"/></svg>',
    controls: '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="2.5" y="6" width="19" height="12" rx="2" fill="none" stroke="currentColor" stroke-width="2"/><path d="M6 10h1M9.5 10h1M13 10h1M16.5 10h1M8 14h8" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>',
    controller: '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3.5" y="3.5" width="17" height="17" rx="2" fill="none" stroke="currentColor" stroke-width="2"/><path d="M8 8h.01M12 8h.01M16 8h.01M8 12h.01M12 12h.01M16 12h.01M8 16h.01M12 16h.01M16 16h.01" stroke="currentColor" stroke-width="3" stroke-linecap="round"/></svg>',
    games: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/></svg>'
  };

  const PAGES = [
    { id: 'settings', label: 'Settings', icon: 'settings' },
    { id: 'controls', label: 'Controls', icon: 'controls' },
    { id: 'controller', label: 'Controller', icon: 'controller' },
    { id: 'games', label: 'Games', icon: 'games' }
  ];

  const esc = (s) => String(s == null ? '' : s)
    .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

  // "Medium (Next Step Highlight)" -> main "Medium", detail "Next Step Highlight"
  function splitLabel(label) {
    const m = /^(.*?)\s*\((.*)\)\s*$/.exec(String(label));
    return m && m[1] ? { main: m[1], detail: m[2] } : { main: String(label), detail: '' };
  }

  class LaunchBandMenu {
    constructor(config = {}) {
      const look = GAME_LOOKS[config.gameId] || {};
      this.config = Object.assign({
        gameId: 'game',
        gameTitle: 'LAUNCHBAND GAME',
        gameSubtitle: 'Hardware Arcade Controller',
        skin: 'default',
        emblem: '◆',
        shortTitle: '',
        kicker: 'LaunchBand',
        playLabel: 'Play',
        resumeLabel: 'Resume',
        restartLabel: 'Restart',
        escPassthrough: null,
        requiredDevices: [
          { id: 'lp', name: 'Novation Launchpad', match: /launchpad/i, required: true }
        ],
        parameters: [],
        keybindings: [],
        defaultInputMode: 'hardware', // 'hardware' | 'keyboard'
        autoShowOnLoad: true,
        onStart: null,
        onResume: null,
        onPause: null,
        onRestart: null,
        onParameterChange: null,
        onDeviceChange: null
      }, look, config);
      this.config.hud = Object.assign({ bar: '#bar', adopt: [] }, look.hud, config.hud);

      this.isHost = window.parent !== window;
      this.storageKey = 'lbm_settings_' + this.config.gameId;
      this.inputMode = this.config.defaultInputMode;
      // The app's sandboxed game frames have no localStorage access.
      try {
        this.inputMode = localStorage.getItem('lbm_input_mode_' + this.config.gameId) ||
          localStorage.getItem('lbm_hub_mode') || this.inputMode;
      } catch (e) {}
      this.settings = this.loadSavedSettings();
      this.isOpen = false;
      this.hasStarted = false;
      this.isTestingLeds = false;
      this.page = 'settings';

      // Hardware state
      this.midiAccess = null;
      this.detectedDevices = new Map(); // id -> { name, input, output, connected }
      this.hostState = 'disconnected';

      // DOM refs
      this.overlayEl = null;
      this.hudEl = null;
      this.inGameBtn = null;
      this.toastsEl = null;

      this.initDOM();
      this.initMIDI();
      this.initEvents();

      if (this.config.autoShowOnLoad) {
        this.open();
      }
    }

    // ────────────────────────── State & Storage ──────────────────────────
    loadSavedSettings() {
      let saved = {};
      try {
        const raw = localStorage.getItem(this.storageKey);
        if (raw) saved = JSON.parse(raw);
      } catch (e) {
        console.warn('[LBM] Failed to load saved settings', e);
      }

      const out = {};
      for (const p of this.config.parameters) {
        let v = (saved[p.id] !== undefined) ? saved[p.id] : p.default;
        // Drop stale values that are no longer offered
        if (p.type === 'select' && !p.options.some(o => String(o.value) === String(v))) v = p.default;
        out[p.id] = v;
      }
      return out;
    }

    saveSettings() {
      try {
        localStorage.setItem(this.storageKey, JSON.stringify(this.settings));
        localStorage.setItem('lbm_input_mode_' + this.config.gameId, this.inputMode);
      } catch (e) {}
    }

    // ────────────────────────── Web MIDI Management ──────────────────────────
    initMIDI() {
      // In host (e.g. Launchpad Drummer iframe)
      if (this.isHost) {
        window.addEventListener('message', (e) => {
          const m = e.data;
          if (!m || e.source !== window.parent) return;
          if (m.lpd === 'status') {
            this.hostState = m.state || 'disconnected';
            this.updateHardwareStatus();
          }
        });
        // Request initial status from host
        try { window.parent.postMessage({ lpd: 'req_status' }, '*'); } catch (e) {}
        this.updateHardwareStatus();
        return;
      }

      // Standalone browser Web MIDI API
      if (typeof navigator === 'undefined' || !navigator.requestMIDIAccess) {
        console.warn('[LBM] Web MIDI API not supported in this browser.');
        this.updateHardwareStatus();
        return;
      }

      const attach = (access) => {
        this.midiAccess = access;
        this.midiAccess.onstatechange = () => this.scanMIDIDevices();
        this.scanMIDIDevices();
      };
      navigator.requestMIDIAccess({ sysex: true }).then(attach, (err) => {
        console.warn('[LBM] MIDI access denied or unavailable with SysEx:', err.message);
        // Retry without sysex as fallback
        navigator.requestMIDIAccess({ sysex: false }).then(attach, (err2) => {
          console.warn('[LBM] MIDI access completely refused:', err2.message);
          this.updateHardwareStatus();
        });
      });
    }

    scanMIDIDevices() {
      if (!this.midiAccess) return;
      this.detectedDevices.clear();

      const inputs = [...this.midiAccess.inputs.values()];
      const outputs = [...this.midiAccess.outputs.values()];

      for (const req of this.config.requiredDevices) {
        const inp = inputs.find(i => req.match.test(i.name || '') && i.state === 'connected');
        const out = outputs.find(o => req.match.test(o.name || '') && o.state === 'connected');
        this.detectedDevices.set(req.id, {
          name: req.name,
          input: inp || null,
          output: out || null,
          connected: Boolean(inp && out)
        });
      }

      this.updateHardwareStatus();
      if (typeof this.config.onDeviceChange === 'function') {
        this.config.onDeviceChange(this.detectedDevices);
      }
    }

    checkHardwareConnected() {
      if (this.isHost) {
        return this.hostState === 'connected';
      }
      if (!this.midiAccess) return false;

      // Check all required devices
      for (const req of this.config.requiredDevices) {
        if (!req.required) continue;
        const dev = this.detectedDevices.get(req.id);
        if (!dev || !dev.connected) {
          return false;
        }
      }
      return true;
    }

    // ────────────────────────── LED Flash Test ──────────────────────────
    flashLedTest() {
      if (this.isTestingLeds) return;
      this.isTestingLeds = true;
      this.toast('Lighting the controller…', 'info');

      if (this.isHost) {
        // Build rainbow burst for host
        const burst = [];
        for (let note = 11; note <= 88; note++) {
          burst.push([note, 63, 20, 40]);
        }
        window.parent.postMessage({ lpd: 'leds', leds: burst }, '*');
        setTimeout(() => {
          const off = [];
          for (let note = 11; note <= 88; note++) off.push([note, 0, 0, 0]);
          window.parent.postMessage({ lpd: 'leds', leds: off }, '*');
          this.isTestingLeds = false;
        }, 1200);
        return;
      }

      // Standalone direct MIDI output
      const lpDev = this.detectedDevices.get('lp');
      const lcDev = this.detectedDevices.get('lcxl');

      if (lpDev && lpDev.output) {
        try {
          const rgbBatch = [];
          for (let r = 1; r <= 8; r++) {
            for (let c = 1; c <= 8; c++) {
              const note = r * 10 + c;
              const red = Math.round(15 + 45 * Math.sin(r * 0.8));
              const green = Math.round(20 + 40 * Math.cos(c * 0.8));
              rgbBatch.push(note, red, green, 60);
            }
          }
          // Launchpad MK2 RGB SysEx F0 00 20 29 02 18 0B [note r g b]... F7
          lpDev.output.send([0xF0, 0x00, 0x20, 0x29, 0x02, 0x18, 0x0B, ...rgbBatch, 0xF7]);
        } catch (e) {
          console.warn('[LBM] LED send error:', e);
        }
      }

      if (lcDev && lcDev.output) {
        try {
          for (let idx = 0; idx <= 39; idx++) {
            lcDev.output.send([0xF0, 0x00, 0x20, 0x29, 0x02, 0x11, 0x78, 0, idx, 0x3C, 0xF7]);
          }
        } catch (e) {}
      }

      setTimeout(() => {
        if (lpDev && lpDev.output) {
          try { lpDev.output.send([0xF0, 0x00, 0x20, 0x29, 0x02, 0x18, 0x0E, 0x00, 0xF7]); } catch (e) {}
        }
        if (lcDev && lcDev.output) {
          try { for (let t = 0; t < 16; t++) lcDev.output.send([0xB0 + t, 0, 0]); } catch (e) {}
        }
        this.isTestingLeds = false;
        this.toast('Controller answered.', 'success');
      }, 1400);
    }

    // ────────────────────────── DOM Rendering ──────────────────────────
    initDOM() {
      const skinClass = 'lbm-skin-' + this.config.skin;

      this.initHUD(skinClass);

      // Main overlay
      this.overlayEl = document.createElement('div');
      this.overlayEl.className = 'lbm-overlay ' + skinClass;
      this.overlayEl.innerHTML = this.renderHTML();
      document.body.appendChild(this.overlayEl);

      // Toast container
      this.toastsEl = document.createElement('div');
      this.toastsEl.className = 'lbm-toast-container ' + skinClass;
      document.body.appendChild(this.toastsEl);

      this.bindDOMEvents();
      this.showPage(this.page);
    }

    // Turn the game's settings header into a slim HUD: menu button, title,
    // and the few live elements worth keeping. Everything else is parked
    // (still in the DOM, so the game keeps reading and listening to it).
    initHUD(skinClass) {
      const hud = this.config.hud;
      const bar = hud.bar && typeof document.querySelector === 'function' ? document.querySelector(hud.bar) : null;

      const menuBtn = document.createElement('button');
      menuBtn.type = 'button';
      menuBtn.className = 'lbm-hud-menu';
      menuBtn.title = 'Menu (Esc)';
      menuBtn.innerHTML = '<span class="lbm-burger" aria-hidden="true"><i></i><i></i><i></i></span><span>Menu</span><kbd>Esc</kbd>';
      menuBtn.addEventListener('click', () => this.toggle());

      if (!bar) {
        // No header to take over: floating button
        menuBtn.id = 'lbm-in-game-btn';
        menuBtn.className += ' ' + skinClass;
        this.inGameBtn = menuBtn;
        document.body.appendChild(menuBtn);
        return;
      }

      const stash = document.createElement('div');
      stash.id = 'lbm-stash';
      stash.hidden = true;
      for (const node of Array.from(bar.childNodes || [])) stash.appendChild(node);
      document.body.appendChild(stash);
      if (this.config.autoParams !== false) this.adoptStashSelects(stash);

      const title = document.createElement('div');
      title.className = 'lbm-hud-title';
      title.innerHTML = `<span class="lbm-emblem-sm" aria-hidden="true">${esc(this.config.emblem)}</span>` +
        `<span>${esc(this.config.shortTitle || this.config.gameTitle)}</span>`;

      const slot = document.createElement('div');
      slot.className = 'lbm-hud-slot';
      for (const sel of hud.adopt) {
        const el = stash.querySelector(sel);
        if (el) slot.appendChild(el);
      }

      bar.appendChild(menuBtn);
      bar.appendChild(title);
      bar.appendChild(slot);
      bar.classList.add('lbm-hud', skinClass);
      this.hudEl = bar;
      this.inGameBtn = menuBtn;
    }

    // Header selects the game config doesn't list still need a home: they become
    // settings that drive the original <select> directly.
    adoptStashSelects(stash) {
      const known = new Set(this.config.parameters.map(p => p.id));
      for (const sel of Array.from(stash.querySelectorAll('select'))) {
        if (!sel.id || known.has(sel.id)) continue;
        const lab = sel.closest('label');
        const text = lab ? Array.from(lab.childNodes).filter(n => n.nodeType === 3).map(n => n.textContent).join(' ') : '';
        this.config.parameters.push({
          id: sel.id,
          label: text.replace(/[:\s]+$/, '').trim() || sel.id,
          type: 'select',
          auto: true,
          default: sel.value,
          options: Array.from(sel.options).map(o => ({ value: o.value, label: o.textContent }))
        });
      }
      this.settings = this.loadSavedSettings();
      for (const p of this.config.parameters) {
        if (p.auto && String(this.settings[p.id]) !== String(p.default)) this.applyParam(p, this.settings[p.id]);
      }
    }

    applyParam(p, val) {
      if (p.auto) {
        const el = document.getElementById(p.id);
        if (el) {
          el.value = val;
          el.dispatchEvent(new Event('change'));
        }
      } else if (typeof this.config.onParameterChange === 'function') {
        this.config.onParameterChange(p.id, val, this.settings);
      }
    }

    renderHTML() {
      const g = this.config;
      const nav = PAGES.map(p => `
        <button type="button" class="lbm-nav-item" data-page="${p.id}" data-col="nav">
          <span class="lbm-ico">${ICONS[p.icon]}</span><span>${p.label}</span>
        </button>`).join('');

      return `
        <div class="lbm-shell" role="dialog" aria-modal="true" aria-label="${esc(g.gameTitle)} menu">
          <aside class="lbm-side">
            <div class="lbm-brand-row">
              <span class="lbm-brand">LaunchBand</span>
              <span class="lbm-kicker">${esc(g.kicker)}</span>
            </div>
            <div class="lbm-emblem" aria-hidden="true">${esc(g.emblem)}</div>
            <h1 class="lbm-title">${esc(g.gameTitle)}</h1>
            <p class="lbm-tagline">${esc(g.gameSubtitle)}</p>

            <div class="lbm-actions">
              <button type="button" class="lbm-cta" id="lbm-btn-start" data-col="nav">
                <span class="lbm-ico">${ICONS.play}</span>
                <span class="lbm-cta-text"><span class="lbm-cta-main"></span><span class="lbm-cta-sub"></span></span>
              </button>
              <button type="button" class="lbm-nav-item lbm-restart" id="lbm-btn-restart" data-col="nav" hidden>
                <span class="lbm-ico">${ICONS.restart}</span><span>${esc(g.restartLabel)}</span>
              </button>
            </div>

            <div class="lbm-mode-switch" role="radiogroup" aria-label="Input">
              <button type="button" role="radio" class="lbm-mode-opt" id="lbm-btn-mode-hw" data-col="nav">Launchpad</button>
              <button type="button" role="radio" class="lbm-mode-opt" id="lbm-btn-mode-kb" data-col="nav">Keyboard &amp; mouse</button>
            </div>
            <div class="lbm-status-line"><span class="lbm-lamp" id="lbm-status-lamp"></span><span id="lbm-status-text"></span></div>

            <nav class="lbm-nav">${nav}</nav>

            <div class="lbm-hint"><kbd>↑</kbd><kbd>↓</kbd> choose · <kbd>←</kbd><kbd>→</kbd> adjust · <kbd>Enter</kbd> select · <kbd>Esc</kbd> back</div>
          </aside>

          <main class="lbm-main">
            <section class="lbm-page" data-page="settings">
              <header class="lbm-page-head"><h2>Settings</h2><span class="lbm-page-note">Saved automatically</span></header>
              <div class="lbm-rows" id="lbm-params-grid">${this.renderParametersHTML()}</div>
              <div class="lbm-page-foot"><button type="button" class="lbm-link-btn" id="lbm-btn-defaults" data-col="page">Restore defaults</button></div>
            </section>

            <section class="lbm-page" data-page="controls">
              <header class="lbm-page-head"><h2>Controls</h2><span class="lbm-page-note">Keyboard · mouse · pads</span></header>
              <div class="lbm-keys">${this.renderKeybindingsHTML()}</div>
            </section>

            <section class="lbm-page" data-page="controller">
              <header class="lbm-page-head"><h2>Controller</h2><span class="lbm-page-note" id="lbm-mode-status-pill"></span></header>
              <div class="lbm-devices" id="lbm-devices-list"></div>
              <div class="lbm-lockout" id="lbm-lockout-banner" hidden>
                <b>No controller found.</b> Plug the Launchpad in by USB, or play with keyboard and mouse.
                <button type="button" class="lbm-link-btn" id="lbm-btn-bypass-kb" data-col="page">Use keyboard &amp; mouse</button>
              </div>
              <div class="lbm-page-foot">
                <button type="button" class="lbm-pill-btn" id="lbm-btn-rescan" data-col="page">Scan again</button>
                <button type="button" class="lbm-pill-btn" id="lbm-btn-led-test" data-col="page">Test lights</button>
              </div>
            </section>

            <section class="lbm-page" data-page="games">
              <header class="lbm-page-head"><h2>Games</h2><span class="lbm-page-note">Switch table</span></header>
              <div class="lbm-games">${this.renderGamesHTML()}</div>
            </section>
          </main>
        </div>
      `;
    }

    renderParametersHTML() {
      if (!this.config.parameters || this.config.parameters.length === 0) {
        return `<p class="lbm-empty">Nothing to set for this game.</p>`;
      }

      return this.config.parameters.map(p => {
        const note = p.note ? `<span class="lbm-row-note">${esc(p.note)}</span>` : '';
        if (p.type === 'toggle') {
          return `
            <div class="lbm-row" data-row="${p.id}">
              <div class="lbm-row-label"><span>${esc(p.label)}</span>${p.desc ? `<span class="lbm-row-note">${esc(p.desc)}</span>` : note}</div>
              <button type="button" class="lbm-switch" role="switch" data-param-id="${p.id}" data-col="page" aria-label="${esc(p.label)}">
                <span class="lbm-switch-off">Off</span><span class="lbm-switch-on">On</span>
              </button>
            </div>`;
        }
        if (p.type === 'select') {
          return `
            <div class="lbm-row" data-row="${p.id}">
              <div class="lbm-row-label"><span>${esc(p.label)}</span>${note}</div>
              <div class="lbm-stepper" data-param-id="${p.id}" data-col="page" tabindex="0" role="spinbutton" aria-label="${esc(p.label)}">
                <button type="button" class="lbm-step" data-dir="-1" tabindex="-1" aria-label="Previous">‹</button>
                <span class="lbm-step-value"><span class="lbm-step-main"></span><span class="lbm-step-detail"></span></span>
                <button type="button" class="lbm-step" data-dir="1" tabindex="-1" aria-label="Next">›</button>
                <span class="lbm-step-dots" aria-hidden="true">${p.options.map(() => '<i></i>').join('')}</span>
              </div>
            </div>`;
        }
        return '';
      }).join('');
    }

    renderKeybindingsHTML() {
      if (!this.config.keybindings || this.config.keybindings.length === 0) {
        return `<p class="lbm-empty">Standard keyboard and mouse controls.</p>`;
      }
      const rows = this.config.keybindings
        // The menu key gets its own row below; skip stale "Esc / M" entries
        .filter(k => !/^esc\b/i.test(k.key))
        .map(k => {
          const keys = String(k.key).split(/\s*\/\s*/).map(part => `<kbd>${esc(part)}</kbd>`).join('<span class="lbm-or">or</span>');
          return `<div class="lbm-key"><span class="lbm-key-caps">${keys}</span><span class="lbm-key-desc">${esc(k.desc)}</span></div>`;
        });
      rows.push(`<div class="lbm-key"><span class="lbm-key-caps"><kbd>Esc</kbd></span><span class="lbm-key-desc">Open or close this menu</span></div>`);
      return rows.join('');
    }

    renderGamesHTML() {
      const cur = this.config.gameId;
      // Hidden (incomplete) games stay out of the list unless they are the one running
      const cards = ALL_GAMES.filter(game => !game.hidden || game.id === cur).map(game => {
        const look = GAME_LOOKS[game.id] || {};
        const isCur = game.id === cur;
        return `
          <button type="button" class="lbm-game-card lbm-skin-${look.skin || 'default'} ${isCur ? 'current' : ''}" data-game="${game.id}" data-col="page" ${isCur ? 'aria-current="true"' : ''}>
            <span class="lbm-game-emblem" aria-hidden="true">${esc(look.emblem || '◆')}</span>
            <span class="lbm-game-text">
              <span class="lbm-game-name">${esc(game.title)}</span>
              <span class="lbm-game-sub">${esc(game.sub)}</span>
            </span>
            <span class="lbm-game-tag">${isCur ? 'Playing' : 'Switch'}</span>
          </button>`;
      }).join('');
      const hub = this.isHost ? '' :
        `<a class="lbm-link-btn lbm-hub-link" href="../index.html" data-col="page">All games (hub)</a>`;
      return cards + hub;
    }

    $(sel) {
      return this.overlayEl.querySelector(sel);
    }

    $all(sel) {
      return Array.from(this.overlayEl.querySelectorAll(sel) || []);
    }

    refreshParameters() {
      for (const p of this.config.parameters) {
        const val = this.settings[p.id];
        if (p.type === 'toggle') {
          const sw = this.$(`.lbm-switch[data-param-id="${p.id}"]`);
          if (sw && sw.setAttribute) sw.setAttribute('aria-checked', String(Boolean(val)));
        } else if (p.type === 'select') {
          const st = this.$(`.lbm-stepper[data-param-id="${p.id}"]`);
          if (!st || !st.querySelector) continue;
          const idx = Math.max(0, p.options.findIndex(o => String(o.value) === String(val)));
          const lbl = splitLabel(p.options[idx].label);
          const main = st.querySelector('.lbm-step-main');
          const detail = st.querySelector('.lbm-step-detail');
          if (main) main.textContent = lbl.main;
          if (detail) detail.textContent = lbl.detail;
          if (st.setAttribute) st.setAttribute('aria-valuetext', p.options[idx].label);
          Array.from(st.querySelectorAll('.lbm-step-dots i') || []).forEach((d, i) => d.classList.toggle('on', i === idx));
        }
      }
    }

    setParam(id, val) {
      this.settings[id] = val;
      this.saveSettings();
      this.refreshParameters();
      const p = this.config.parameters.find(x => x.id === id);
      if (p) this.applyParam(p, val);
    }

    stepParam(id, dir) {
      const p = this.config.parameters.find(x => x.id === id);
      if (!p) return;
      if (p.type === 'toggle') {
        this.setParam(id, !this.settings[id]);
        return;
      }
      const n = p.options.length;
      const idx = Math.max(0, p.options.findIndex(o => String(o.value) === String(this.settings[id])));
      this.setParam(id, p.options[(idx + dir + n) % n].value);
    }

    setMode(m, quiet) {
      this.inputMode = m;
      this.saveSettings();
      this.updateHardwareStatus();
      if (!quiet) this.toast(m === 'hardware' ? 'Input: Launchpad' : 'Input: keyboard & mouse', 'info');
    }

    showPage(id) {
      this.page = id;
      for (const s of this.$all('.lbm-page')) s.classList.toggle('active', s.getAttribute('data-page') === id);
      for (const b of this.$all('.lbm-nav-item[data-page]')) b.classList.toggle('active', b.getAttribute('data-page') === id);
    }

    bindDOMEvents() {
      // Mode switch
      const hw = this.$('#lbm-btn-mode-hw');
      const kb = this.$('#lbm-btn-mode-kb');
      const bypass = this.$('#lbm-btn-bypass-kb');
      if (hw) hw.onclick = () => this.setMode('hardware');
      if (kb) kb.onclick = () => this.setMode('keyboard');
      if (bypass) bypass.onclick = () => this.setMode('keyboard');

      // Pages
      for (const b of this.$all('.lbm-nav-item[data-page]')) {
        b.onclick = () => this.showPage(b.getAttribute('data-page'));
        b.onfocus = () => this.showPage(b.getAttribute('data-page'));
      }

      // Hardware actions
      const rescan = this.$('#lbm-btn-rescan');
      const ledTest = this.$('#lbm-btn-led-test');
      if (rescan) rescan.onclick = () => {
        this.toast('Scanning for controllers…', 'info');
        this.scanMIDIDevices();
      };
      if (ledTest) ledTest.onclick = () => this.flashLedTest();

      // Parameters
      for (const st of this.$all('.lbm-stepper[data-param-id]')) {
        const id = st.getAttribute('data-param-id');
        for (const b of Array.from(st.querySelectorAll('.lbm-step') || [])) {
          b.onclick = (e) => { e.stopPropagation(); this.stepParam(id, +b.getAttribute('data-dir')); st.focus(); };
        }
        st.onclick = () => this.stepParam(id, 1);
      }
      for (const sw of this.$all('.lbm-switch[data-param-id]')) {
        sw.onclick = () => this.stepParam(sw.getAttribute('data-param-id'), 1);
      }
      this.refreshParameters();

      // Restore defaults
      const defBtn = this.$('#lbm-btn-defaults');
      if (defBtn) defBtn.onclick = () => {
        for (const p of this.config.parameters) this.settings[p.id] = p.default;
        this.saveSettings();
        this.refreshParameters();
        this.toast('Defaults restored', 'info');
        for (const p of this.config.parameters) this.applyParam(p, p.default);
      };

      // Restart
      const restartBtn = this.$('#lbm-btn-restart');
      if (restartBtn) restartBtn.onclick = () => {
        this.close(true);
        if (typeof this.config.onRestart === 'function') {
          this.config.onRestart(this.settings, this.inputMode);
        }
      };

      // Play / Resume
      const startBtn = this.$('#lbm-btn-start');
      if (startBtn) startBtn.onclick = () => this.primaryAction();

      // Game switcher
      for (const card of this.$all('.lbm-game-card[data-game]')) {
        card.onclick = () => {
          const target = ALL_GAMES.find(x => x.id === card.getAttribute('data-game'));
          if (!target || target.id === this.config.gameId) return;
          this.toast(`Loading ${target.title}…`, 'info');
          setTimeout(() => { window.location.href = this.isHost ? target.hostPath : target.path; }, 250);
        };
      }
    }

    primaryAction() {
      if (this.hasStarted) {
        this.close();
        return;
      }
      if (!this.canStartGame()) {
        // No controller: one press falls back to keyboard play instead of locking the player out
        this.setMode('keyboard', true);
        this.toast('No controller found: playing with keyboard & mouse', 'warn');
      }
      this.hasStarted = true;
      this.close(true);
      this.updateButtonVisibility();
      if (typeof this.config.onStart === 'function') {
        this.config.onStart(this.settings, this.inputMode);
      }
    }

    initEvents() {
      // Capture phase: the menu sees keys before the game, and swallows them while open
      window.addEventListener('keydown', (e) => {
        if (this.isOpen) {
          this.handleMenuKey(e);
          if (e.stopImmediatePropagation) e.stopImmediatePropagation();
          return;
        }
        if (e.code !== 'Escape' || e.repeat) return;
        const tag = (e.target && e.target.tagName) || '';
        if (tag === 'INPUT' || tag === 'TEXTAREA') return;
        if (typeof this.config.escPassthrough === 'function' && this.config.escPassthrough()) return;
        e.preventDefault();
        if (e.stopImmediatePropagation) e.stopImmediatePropagation();
        this.open();
      }, true);
    }

    // Two columns: left (play, input, pages) and right (current page). Up/Down move
    // within a column, Right enters the page, Esc/Left steps back out.
    handleMenuKey(e) {
      const active = document.activeElement;
      const col = active && active.getAttribute ? active.getAttribute('data-col') : null;
      const visible = (el) => el.offsetParent !== null && !el.hidden;
      const navItems = this.$all('[data-col="nav"]').filter(visible);
      const pageItems = this.$all('.lbm-page.active [data-col="page"]').filter(visible);
      const list = col === 'page' ? pageItems : navItems;
      const i = list.indexOf(active);

      switch (e.code) {
        case 'ArrowDown':
        case 'ArrowUp': {
          e.preventDefault();
          if (!list.length) return;
          const d = e.code === 'ArrowDown' ? 1 : -1;
          const next = list[i < 0 ? 0 : (i + d + list.length) % list.length];
          next.focus();
          if (next.scrollIntoView) next.scrollIntoView({ block: 'nearest' });
          return;
        }
        case 'ArrowRight':
        case 'ArrowLeft': {
          e.preventDefault();
          const d = e.code === 'ArrowRight' ? 1 : -1;
          if (col === 'page' && active.getAttribute('data-param-id')) {
            this.stepParam(active.getAttribute('data-param-id'), d);
          } else if (col === 'page' && d < 0) {
            this.focusNavForPage();
          } else if (col === 'nav' && active.classList.contains('lbm-mode-opt')) {
            const other = active.id === 'lbm-btn-mode-hw' ? 'keyboard' : 'hardware';
            this.setMode(other);
            this.$(other === 'hardware' ? '#lbm-btn-mode-hw' : '#lbm-btn-mode-kb').focus();
          } else if (d > 0 && pageItems.length) {
            pageItems[0].focus();
          }
          return;
        }
        case 'Escape':
          e.preventDefault();
          if (col === 'page') this.focusNavForPage();
          else if (this.hasStarted) this.close();
          else { const s = this.$('#lbm-btn-start'); if (s) s.focus(); }
          return;
        default:
          // Enter / Space reach the focused button natively; with nothing focused, Enter plays
          if ((!active || active === document.body) && e.code === 'Enter') {
            e.preventDefault();
            this.primaryAction();
          }
      }
    }

    focusNavForPage() {
      const b = this.$(`.lbm-nav-item[data-page="${this.page}"]`);
      if (b) b.focus();
    }

    // ────────────────────────── Connection & Lockout Logic ──────────────────────────
    canStartGame() {
      if (this.inputMode === 'keyboard') {
        return true; // Bypass device check
      }
      return this.checkHardwareConnected();
    }

    updateHardwareStatus() {
      if (!this.overlayEl) return;
      const isConnected = this.checkHardwareConnected();
      const isHwMode = this.inputMode === 'hardware';
      const isReady = this.canStartGame();
      const g = this.config;

      const hw = this.$('#lbm-btn-mode-hw');
      const kb = this.$('#lbm-btn-mode-kb');
      if (hw && hw.setAttribute) hw.setAttribute('aria-checked', String(isHwMode));
      if (kb && kb.setAttribute) kb.setAttribute('aria-checked', String(!isHwMode));

      // Status line under the mode switch
      let state, msg;
      if (!isHwMode) { state = 'kb'; msg = 'Keyboard & mouse — no controller needed'; }
      else if (isConnected) { state = 'ok'; msg = 'Controller connected'; }
      else { state = 'missing'; msg = 'Controller not found'; }
      const lamp = this.$('#lbm-status-lamp');
      const text = this.$('#lbm-status-text');
      if (lamp) lamp.className = 'lbm-lamp ' + state;
      if (text) text.textContent = msg;

      const pill = this.$('#lbm-mode-status-pill');
      if (pill) pill.textContent = msg;

      // Device rows
      const listEl = this.$('#lbm-devices-list');
      if (listEl) {
        const row = (ok, name, ports) => `
          <div class="lbm-device">
            <span class="lbm-lamp ${ok ? 'ok' : (isHwMode ? 'missing' : 'kb')}"></span>
            <span class="lbm-device-text"><span class="lbm-device-name">${esc(name)}</span><span class="lbm-device-ports">${esc(ports)}</span></span>
            <span class="lbm-device-badge ${ok ? 'ok' : (isHwMode ? 'missing' : 'kb')}">${ok ? 'Online' : (isHwMode ? 'Missing' : 'Not needed')}</span>
          </div>`;
        if (this.isHost) {
          listEl.innerHTML = row(this.hostState === 'connected', 'Launchpad Drummer bridge', 'Host port: ' + this.hostState);
        } else {
          listEl.innerHTML = g.requiredDevices.map(req => {
            const dev = this.detectedDevices.get(req.id);
            const ok = Boolean(dev && dev.connected);
            const ports = dev ? `In: ${dev.input ? dev.input.name : '—'} · Out: ${dev.output ? dev.output.name : '—'}` : 'Waiting for MIDI access';
            return row(ok, req.name, ports);
          }).join('');
        }
      }

      const banner = this.$('#lbm-lockout-banner');
      if (banner) banner.hidden = !(isHwMode && !isConnected);

      // Primary button
      const startBtn = this.$('#lbm-btn-start');
      if (startBtn && startBtn.querySelector) {
        const main = startBtn.querySelector('.lbm-cta-main');
        const sub = startBtn.querySelector('.lbm-cta-sub');
        let mainTxt, subTxt;
        if (this.hasStarted) { mainTxt = g.resumeLabel; subTxt = 'Esc'; }
        else if (isReady) { mainTxt = g.playLabel; subTxt = isHwMode ? 'With Launchpad' : 'With keyboard & mouse'; }
        else { mainTxt = g.playLabel; subTxt = 'No controller — keyboard & mouse'; }
        if (main) main.textContent = mainTxt;
        if (sub) sub.textContent = subTxt;
        startBtn.classList.toggle('fallback', !isReady && !this.hasStarted);
      }

      // HUD menu button lamp
      if (this.inGameBtn && this.inGameBtn.setAttribute) this.inGameBtn.setAttribute('data-state', state);
    }

    updateButtonVisibility() {
      const restartBtn = this.$('#lbm-btn-restart');
      if (restartBtn) restartBtn.hidden = !this.hasStarted;
      if (this.overlayEl) this.overlayEl.classList.toggle('started', this.hasStarted);
      this.updateHardwareStatus();
    }

    // ────────────────────────── Public Controls ──────────────────────────
    open() {
      this.isOpen = true;
      this.updateButtonVisibility();
      this.overlayEl.classList.add('active');
      if (document.body && document.body.classList) document.body.classList.add('lbm-menu-open');
      const start = this.$('#lbm-btn-start');
      if (start && start.focus) setTimeout(() => { if (this.isOpen) start.focus({ preventScroll: true }); }, 30);
      if (this.hasStarted && typeof this.config.onPause === 'function') {
        this.config.onPause();
      }
    }

    // silent: closing to start or restart, so don't fire onResume
    close(silent) {
      this.isOpen = false;
      this.overlayEl.classList.remove('active');
      if (document.body && document.body.classList) document.body.classList.remove('lbm-menu-open');
      const a = document.activeElement;
      if (a && a.blur && this.overlayEl.contains && this.overlayEl.contains(a)) a.blur();
      if (!silent && this.hasStarted && typeof this.config.onResume === 'function') {
        this.config.onResume(this.settings, this.inputMode);
      }
    }

    toggle() {
      if (this.isOpen) {
        if (this.hasStarted) this.close();
      } else {
        this.open();
      }
    }

    toast(text, type = 'info') {
      if (!this.toastsEl) return;
      const t = document.createElement('div');
      t.className = `lbm-toast ${type}`;
      t.textContent = text;
      this.toastsEl.appendChild(t);
      setTimeout(() => {
        t.style.opacity = '0';
        t.style.transition = 'opacity 0.25s ease';
        setTimeout(() => t.remove(), 250);
      }, 2600);
    }

    getSettings() {
      return Object.assign({}, this.settings);
    }

    getInputMode() {
      return this.inputMode;
    }
  }

  // Export to window
  global.LaunchBandMenu = LaunchBandMenu;
  global.LaunchBandGames = ALL_GAMES;

})(typeof window !== 'undefined' ? window : this);
