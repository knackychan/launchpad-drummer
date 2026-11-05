"""Run after npm run build. Uses the environment's Playwright + installed Edge."""
from pathlib import Path
import functools
import http.server
import json
import threading
from playwright.sync_api import sync_playwright

REPO = Path(__file__).resolve().parents[1]
ROOT = REPO.parent
OUT = REPO / '.verification/sector'
OUT.mkdir(parents=True, exist_ok=True)
CSP = json.loads((REPO / 'src-tauri/tauri.conf.json').read_text())['app']['security']['csp']
PROTOTYPE = ROOT / 'prototypes/sector/index.html'
if PROTOTYPE.exists():
    for file in ['index.html', 'rules.js', 'game.js', 'sector.css']:
        assert (PROTOTYPE.parent / file).read_bytes() == (REPO / 'public/games/sector' / file).read_bytes(), file


class Handler(http.server.SimpleHTTPRequestHandler):
    def handle(self):
        try:
            super().handle()
        except (ConnectionAbortedError, ConnectionResetError, BrokenPipeError):
            pass  # A tab switch cancels the old game's asset requests.

    def end_headers(self):
        self.send_header('Content-Security-Policy', CSP)
        super().end_headers()

    def log_message(self, *args):
        pass


def serve(folder):
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), functools.partial(Handler, directory=str(folder)))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def wait(frame, expression):
    frame.wait_for_function('() => (' + expression + ')')


def observe(frame):
    wait(frame, 'window.SectorRules && window.gameMenu')
    frame.evaluate('''() => {
      const tick = SectorRules.tick;
      SectorRules.tick = (g, dt) => { window.testState = g; tick(g, dt); };
    }''')
    wait(frame, 'window.testState')


def target(frame, kind='scout', x=.5, y=.5):
    frame.evaluate('''([kind, x, y]) => {
      const g = testState;
      g.paused = false; g.cooldown = 0; g.remaining = 0;
      g.enemies = [{ id: 99, kind, x, y, hp: SectorRules.TYPES[kind].hp, speed: 0, angle: 0 }];
    }''', [kind, x, y])


built = serve(REPO / 'dist')
try:
    with sync_playwright() as p:
        browser = p.chromium.launch(channel='msedge', headless=True)
        # Standalone delivery works offline and without MIDI permissions.
        for location, url in [
            ('standalone-offline', (PROTOTYPE if PROTOTYPE.exists() else REPO / 'public/games/sector/index.html').as_uri()),
            ('production', f'http://127.0.0.1:{built.server_port}/games/sector/index.html'),
        ]:
            context = browser.new_context(viewport={'width': 1280, 'height': 800}, reduced_motion='reduce')
            context.add_init_script("navigator.requestMIDIAccess = () => Promise.reject(new Error('test: no device'))")
            if location.endswith('offline'):
                context.set_offline(True)
            page = context.new_page()
            errors = []
            page.on('pageerror', lambda error: errors.append(str(error)))
            page.goto(url)
            observe(page)
            page.locator('#lbm-btn-mode-kb').click()
            page.locator('#lbm-btn-start').click()
            wait(page, "testState.mode === 'play' && !testState.paused")
            target(page)
            page.locator('#mirror button').first.click()
            wait(page, 'testState.score === 100')
            target(page, x=4.5, y=1.5)
            page.locator('#arena').focus()
            page.keyboard.press('ArrowRight')
            assert page.evaluate('testState.aim.x === 1')  # First shot aimed A1.
            page.evaluate('testState.aim = { x: 4, y: 1 }')
            page.keyboard.press('Enter')
            wait(page, 'testState.kills === 2')
            page.keyboard.press('p')
            wait(page, 'testState.paused')
            before = page.evaluate('testState.time')
            page.wait_for_timeout(200)
            assert page.evaluate('testState.time') == before
            page.keyboard.press('p')
            page.keyboard.press('Escape')
            wait(page, 'gameMenu.isOpen && testState.paused')
            page.locator('#lbm-btn-start').click()
            target(page, 'rock', 4, 4)
            page.evaluate('testState.hp = 1')
            page.locator('#end').wait_for(state='visible')
            page.locator('#retry').click()
            wait(page, "testState.mode === 'play' && testState.hp === 100")
            page.wait_for_timeout(7000)
            page.screenshot(path=str(OUT / f'{location}.png'))
            assert not errors, errors
            print(location, 'mouse / keyboard / pause / loss / restart PASS', flush=True)
            if location == 'production':
                page.set_viewport_size({'width': 390, 'height': 844})
                page.screenshot(path=str(OUT / 'mobile.png'), full_page=True)
                assert page.evaluate('document.documentElement.scrollWidth <= innerWidth')
            context.close()

        # Standalone Web MIDI: layout initialization, input binding, RGB and palette fallback.
        for sysex in [True, False]:
            context = browser.new_context(viewport={'width': 1280, 'height': 800})
            context.add_init_script('''
              window.sentMidi = [];
              window.mockInput = { id: 'in', name: 'Launchpad MK2 16', state: 'connected', onmidimessage: null };
              window.mockOutput = { id: 'out', name: 'Launchpad MK2 16', state: 'connected', send: bytes => sentMidi.push(bytes) };
              window.mockAccess = { sysexEnabled: SYSEX, inputs: new Map([['in', mockInput]]), outputs: new Map([['out', mockOutput]]) };
              navigator.requestMIDIAccess = () => Promise.resolve(mockAccess);
            '''.replace('SYSEX', str(sysex).lower()))
            page = context.new_page()
            errors = []
            page.on('pageerror', lambda error: errors.append(str(error)))
            page.goto(f'http://127.0.0.1:{built.server_port}/games/sector/index.html')
            observe(page)
            wait(page, 'mockInput.onmidimessage && gameMenu.checkHardwareConnected()')
            page.evaluate('mockInput.onmidimessage({ data: new Uint8Array([176,108,127]) })')
            wait(page, "testState.mode === 'play'")
            target(page)
            if sysex:
                assert page.evaluate('sentMidi.some(m => JSON.stringify(m) === JSON.stringify([240,0,32,41,2,24,34,0,247]))')
                wait(page, 'sentMidi.some(m => m[6] === 11 && m.some((v,i) => i >= 7 && (i-7)%4 === 0 && v === 81 && m[i+1] === 10 && m[i+2] === 39 && m[i+3] === 63))')
            else:
                wait(page, 'sentMidi.some(m => m[0] === 144 && m[1] === 81 && m[2] === 45)')
                assert page.evaluate('sentMidi.every(m => m.length === 3)')
            page.evaluate('mockInput.onmidimessage({ data: new Uint8Array([144,81,127]) })')
            wait(page, 'testState.score === 100')
            page.evaluate("mockInput.state = mockOutput.state = 'disconnected'; mockAccess.onstatechange()")
            wait(page, 'testState.paused && gameMenu.isOpen && mockInput.onmidimessage === null')
            page.evaluate("mockInput.state = mockOutput.state = 'connected'; mockAccess.onstatechange()")
            wait(page, 'mockInput.onmidimessage !== null')
            page.evaluate('mockInput.onmidimessage({ data: new Uint8Array([176,108,127]) })')
            wait(page, '!testState.paused && !gameMenu.isOpen')
            if sysex:
                assert page.evaluate('sentMidi.filter(m => m[6] === 34).length === 2')
            assert not errors, errors
            print('Web MIDI', 'RGB SysEx' if sysex else 'palette fallback', 'input / layout / hot-plug PASS', flush=True)
            context.close()

        # Exercise the real production app iframe under the production CSP.
        page = browser.new_page(viewport={'width': 1440, 'height': 900})
        errors = []
        page.on('pageerror', lambda error: errors.append(str(error)))
        page.goto(f'http://127.0.0.1:{built.server_port}/')
        page.get_by_role('button', name='GAMES', exact=True).click()
        page.get_by_role('tab', name='SECTOR DEFENSE', exact=False).click()
        element = page.locator('iframe.prototype')
        frame = element.element_handle().content_frame()
        observe(frame)
        page.evaluate('''() => {
          window.ledFrames = [];
          addEventListener('message', e => { if (e.data?.lpd === 'leds') ledFrames.push(e.data.leds); });
        }''')

        def bridge(message):
            page.evaluate("m => document.querySelector('iframe.prototype').contentWindow.postMessage(m, '*')", message)

        def midi(*data):
            bridge({'lpd': 'midi', 'data': list(data)})

        bridge({'lpd': 'status', 'state': 'connected'})
        midi(0xb0, 108, 127)
        wait(frame, "testState.mode === 'play' && !gameMenu.isOpen")
        target(frame)
        wait(page, 'ledFrames.some(f => f.some(l => l[0] === 81 && l[1] === 10 && l[2] === 39 && l[3] === 63))')
        midi(0x80, 81, 127)  # Note Off must not shoot.
        midi(0x90, 81, 0)
        midi(0xb0, 81, 127)  # Grid notes are not CC controls.
        frame.evaluate("dispatchEvent(new MessageEvent('message', { source: window, data: { lpd: 'midi', data: [144,81,127] } }))")
        assert frame.evaluate('testState.shots') == 0
        midi(0x90, 81, 127)
        wait(frame, 'testState.score === 100')
        target(frame, 'rock', 7.5, 7.5)
        midi(0x90, 18, 127)
        wait(frame, 'testState.enemies[0].hp === 1')
        wait(frame, 'testState.cooldown === 0')
        midi(0x90, 18, 127)
        wait(frame, 'testState.enemies.length === 0')
        target(frame, 'armored', 2.5, 3.5)
        frame.evaluate('testState.charge = 100')
        midi(0x90, 89, 127)
        wait(frame, 'testState.enemies.length === 0 && testState.charge === 0')
        midi(0x90, 79, 127)
        wait(frame, 'testState.paused')
        midi(0xb0, 108, 127)
        wait(frame, '!testState.paused')
        bridge({'lpd': 'status', 'state': 'disconnected'})
        wait(frame, 'testState.paused && gameMenu.isOpen')
        bridge({'lpd': 'status', 'state': 'connected'})
        midi(0xb0, 108, 127)
        wait(frame, '!testState.paused && !gameMenu.isOpen')
        frame.evaluate('gameMenu.open()')
        frame.locator('#lbm-btn-restart').click()
        page.wait_for_timeout(7000)
        page.screenshot(path=str(OUT / 'app.png'))
        page.set_viewport_size({'width': 1280, 'height': 720})
        page.screenshot(path=str(OUT / 'app-1280.png'))
        assert frame.evaluate('document.documentElement.scrollWidth <= innerWidth')
        page.get_by_role('tab', name='CHESS', exact=False).click()
        wait(page, 'ledFrames.some(f => f.length === 80 && f.every(l => l.slice(1).every(v => v === 0)))')
        assert not errors, errors
        print('production iframe: MIDI mapping / releases / message source / RGB / pulse / pause / reconnect / cleanup PASS', flush=True)
        browser.close()
finally:
    built.shutdown()
