"""Smoke-test the built app, or its running WebView2 via --cdp http://127.0.0.1:9415."""
from pathlib import Path
import argparse
import functools
import http.server
import json
import threading
from playwright.sync_api import sync_playwright

REPO = Path(__file__).resolve().parents[1]
OUT = REPO / '.verification/dustline'
CSP = json.loads((REPO / 'src-tauri/tauri.conf.json').read_text())['app']['security']['csp']


class Handler(http.server.SimpleHTTPRequestHandler):
    def handle(self):
        try:
            super().handle()
        except (ConnectionAbortedError, ConnectionResetError, BrokenPipeError):
            pass  # Switching tabs cancels the old frame's requests.

    def end_headers(self):
        self.send_header('Content-Security-Policy', CSP)
        super().end_headers()

    def log_message(self, *args):
        pass


def wait(frame, expression):
    frame.wait_for_function('() => (' + expression + ')')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--cdp')
    args = ap.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    server = None
    try:
        with sync_playwright() as p:
            if args.cdp:
                browser = p.chromium.connect_over_cdp(args.cdp)
                page = next(page for ctx in browser.contexts for page in ctx.pages if '1420' in page.url or 'tauri.localhost' in page.url)
                label = 'native'
            else:
                server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), functools.partial(Handler, directory=str(REPO / 'dist')))
                threading.Thread(target=server.serve_forever, daemon=True).start()
                browser = p.chromium.launch(channel='msedge', headless=True)
                page = browser.new_page(viewport={'width': 1280, 'height': 720})
                page.goto(f'http://127.0.0.1:{server.server_port}/')
                label = 'production'
            errors = []
            page.on('pageerror', lambda e: errors.append(str(e)))
            actual_devices = page.evaluate("__TAURI__.core.invoke('midi_devices')") if args.cdp else []
            key = 'lpd.game.dustline.bridge-check'
            previous = page.evaluate('key=>localStorage.getItem(key)', key)
            midi_key = 'lpd.game.dustline.dustline-midi-v2'
            previous_midi = page.evaluate('key=>localStorage.getItem(key)', midi_key)
            led_batches = []
            if args.cdp:
                def capture_leds(response):
                    if response.url.endswith('/send_lcxl_leds') and response.ok:
                        led_batches.append(json.loads(response.request.post_data)['messages'])
                page.on('response', capture_leds)
            def wait_leds(predicate):
                for _ in range(100):
                    if predicate(led_batches):
                        return
                    page.wait_for_timeout(50)
                assert predicate(led_batches), led_batches
            try:
                page.get_by_role('button', name='GAMES', exact=True).click()
                if args.cdp:
                    page.get_by_role('tab', name='CHESS', exact=False).click()
                page.get_by_role('tab', name='DUSTLINE', exact=False).click()
                def game():
                    f = page.locator('iframe.prototype').element_handle().content_frame()
                    wait(f, 'window.Dustline && document.getElementById("loading").hidden')
                    assert f.evaluate('Dustline.landscape.lapLength') == 18000, 'Packaged Dustline assets are stale'
                    assert f.evaluate('Dustline.landscape.length') == 54000
                    assert f.evaluate('Dustline.raceMode') == 'standard', 'Packaged race menu is stale'
                    assert f.locator('#pauseOverlay').evaluate('e=>e.tagName') == 'DIALOG'
                    assert f.evaluate('''()=>{
                      const sim=new DustlineSimulation(7);sim.reset('demo');sim.state.opponents=[];
                      Object.assign(sim.state,{time:10,heat:.7,eventReadyAt:0});sim.updateTiming(0);
                      return sim.state.event?.scheduledCooling===true && sim.state.event.mode!=='align';
                    }'''), 'Packaged QTE scheduler is stale'
                    return f
                frame = game()
                if args.cdp:
                    frame.evaluate('''()=>{
                      const original=DustlineSimulation.prototype.step;
                      DustlineSimulation.prototype.step=function(...args){window.__dustlineTestSim=this;return original.apply(this,args);};
                    }''')
                    wait(frame, '!!window.__dustlineTestSim')
                assert frame.evaluate('window.origin') == 'null'
                assert frame.evaluate('Dustline.version') == '0.12.0'
                frame.locator('#startBtn').click()
                wait(frame, 'Dustline.audio.decodedCount===28 && Dustline.audio.state==="running"')
                assert frame.evaluate('document.activeElement.id') == 'game', 'Start focuses the racing frame'
                page.keyboard.press('p');wait(frame, 'Dustline.paused')
                page.keyboard.press('p');wait(frame, '!Dustline.paused')

                frame.evaluate('localStorage.setItem("bridge-check", "persisted")')
                wait(page, 'localStorage.getItem("lpd.game.dustline.bridge-check")==="persisted"')
                assert page.locator('iframe.prototype').element_handle().content_frame() == frame
                assert not frame.locator('#intro').is_visible(), 'Saving settings does not restart the race'
                page.evaluate("postMessage({lpd:'store-set',key:'bridge-check',value:'spoofed'}, '*')")
                page.wait_for_timeout(50)
                assert page.evaluate('key=>localStorage.getItem(key)', key) == 'persisted'

                if args.cdp:
                    page.evaluate("__TAURI__.event.emit('midi-devices',[{name:'Launch Control XL',role:'lcxl',status:'connected'}])")
                    wait(frame, 'Dustline.controls.connected')
                    page.evaluate("async()=>{await __TAURI__.event.emit('lcxl-midi',{data:[176,77,32]});await __TAURI__.event.emit('lcxl-midi',{data:[176,84,110]});}")
                    wait(frame, 'Dustline.controls.pair.ready && Dustline.controls.pair.left===32/127 && Dustline.controls.pair.right===110/127')
                    frame.locator('#settingsBtn').click()
                    frame.locator('#xlDefaultsBtn').click()
                    wait_leds(lambda batches: any(m[0] == 240 and m[6] == 119 and m[7] == 0 for b in batches for m in b))
                    wait_leds(lambda batches: any(m[0] == 240 and m[6] == 120 and len(m) == 57 for b in batches for m in b))
                    wait_leds(lambda batches: any(m[0] == 144 and m[1] == 73 for b in batches for m in b))
                    assert 'LED template User 1' in frame.locator('#midiMessage').inner_text()
                    frame.locator('#closeSettingsBtn').click()
                    wait(frame, '!Dustline.paused && !document.getElementById("settingsDialog").open')
                    page.evaluate("async()=>{for(const n of [77,84])await __TAURI__.event.emit('lcxl-midi',{data:[176,n,95]});}")
                    wait(frame, 'Dustline.controls.pair.left===95/127 && Dustline.controls.pair.right===95/127 && Dustline.controls.pair.boostReady')
                    page.evaluate("async()=>{for(const n of [77,84])await __TAURI__.event.emit('lcxl-midi',{data:[176,n,127]});}")
                    wait(frame, 'Dustline.state.boostActive && Dustline.state.boostIntensity===1')
                    assert frame.locator('#leftThrustPercent').inner_text()=='+6.0 dB'
                    assert frame.locator('#rightThrustPercent').inner_text()=='+6.0 dB'
                    page.evaluate("__TAURI__.event.emit('lcxl-midi',{data:[144,92,127]})")
                    wait(frame, '!Dustline.state.boostActive')
                    page.evaluate("async()=>{await __TAURI__.event.emit('lcxl-midi',{data:[128,92,0]});for(const n of [77,84])await __TAURI__.event.emit('lcxl-midi',{data:[176,n,95]});}")
                    assert frame.evaluate('''()=>{
                      const sim=__dustlineTestSim,s=sim.state;
                      Object.assign(s,{countdown:0,time:30,distance:3820,x:0,speed:100,heatLeft:.78,heatRight:.3,charge:.48,
                        recovery:0,offCourse:0,lastImpactAt:0,hazardActive:null,event:null,eventResult:null,knobs:{},powerLeft:.75,powerRight:.75});
                      s.heading=sim.sampleTrack(s.distance).heading;for(const o of s.opponents)o.distance=s.distance+1500;
                      return sim.startAlignment('vent','coolant');
                    }''')
                    target=frame.evaluate('Dustline.state.event.target')
                    page.evaluate("async target=>{for(const v of [0,target])await __TAURI__.event.emit('lcxl-midi',{data:[176,13,v]});}", target)
                    wait(frame, 'Dustline.state.event && Dustline.state.event.aligned')
                    wait_leds(lambda batches:any(m[0]==240 and m[6]==120 and m[7]==0 and m[9]==60 for b in batches for m in b))
                    wait(frame, 'Dustline.state.eventResult && Dustline.state.eventResult.grade==="PERFECT"')
                    for kind, system, note in [('vent', 'coolant', 41), ('repair', 'stabilizer', 73), ('repair', 'hull', 73)]:
                        assert frame.evaluate('''([kind,system])=>{
                          const sim=__dustlineTestSim,s=sim.state;
                          Object.assign(s,{countdown:0,time:s.time+10,distance:3820,x:0,speed:100,heatLeft:.78,heatRight:.3,
                            recovery:0,offCourse:0,lastImpactAt:0,hazardActive:null,event:null,eventResult:null,knobs:{}});
                          s.heading=sim.sampleTrack(s.distance).heading;for(const o of s.opponents)o.distance=s.distance+1500;
                          if(kind==='repair'){s.pendingDamage=system;sim._penaltyTime=8;}
                          return sim.startAlignment(kind,system);
                        }''', [kind, system])
                        await_button = "async n=>{await __TAURI__.event.emit('lcxl-midi',{data:[128,n,0]});await __TAURI__.event.emit('lcxl-midi',{data:[144,n,127]});}"
                        page.evaluate(await_button, note)
                        wait(frame, 'Dustline.state.event && Dustline.state.event.fromAlignment')
                        for _ in range(2 if kind == 'repair' else 1):
                            page.evaluate("n=>__TAURI__.event.emit('lcxl-midi',{data:[128,n,0]})", note)
                            frame.evaluate('''()=>{const s=__dustlineTestSim.state;s.time+=2;s.event.elapsed=s.event.target;s.eventResult=null;}''')
                            page.evaluate("n=>__TAURI__.event.emit('lcxl-midi',{data:[144,n,127]})", note)
                            wait(frame, 'Dustline.state.eventResult')
                            observed = frame.evaluate('({result:Dustline.state.eventResult,event:Dustline.state.event,actionAt:__dustlineTestSim._actionAt,time:Dustline.state.time})')
                            assert observed['result']['grade'] in ['HIT', 'PERFECT', 'GOOD'], (kind, system, observed)
                        wait(frame, 'Dustline.state.eventResult && ["PERFECT","GOOD"].includes(Dustline.state.eventResult.grade)')
                        page.evaluate("n=>__TAURI__.event.emit('lcxl-midi',{data:[128,n,0]})", note)
                    assert page.evaluate('''async()=>{
                      try{await __TAURI__.core.invoke('send_lcxl_leds',{messages:[[176,77,127]]});return false}
                      catch{return true}
                    }'''), 'Native output rejects non-LED MIDI'

                page.get_by_role('button', name='⛶ FULL SCREEN', exact=True).click()
                wait(page, '!!document.fullscreenElement')
                assert page.locator('.games-stage').evaluate('e=>e.classList.contains("full")')
                page.screenshot(path=str(OUT / f'{label}-fullscreen.png'))
                page.get_by_role('button', name='✕ EXIT · ESC', exact=True).click()
                wait(page, '!document.fullscreenElement')
                # Returning from host fullscreen pauses on blur; resume the modal before using the HUD.
                if frame.locator('#pauseOverlay').is_visible():
                    frame.locator('#resumeBtn').click()
                frame.locator('#fullscreenBtn').click()
                wait(frame, '!!document.fullscreenElement')
                frame.evaluate('document.exitFullscreen()')
                wait(page, '!document.fullscreenElement')
                box = page.locator('iframe.prototype').bounding_box()
                viewport = page.evaluate('({width:innerWidth,height:innerHeight})')
                assert box['width'] > 700 and box['height'] > 400, box
                assert box['x'] >= 0 and box['x'] + box['width'] <= viewport['width'] + 1
                assert box['y'] >= 0 and box['y'] + box['height'] <= viewport['height'] + 1
                assert page.evaluate('document.documentElement.scrollWidth<=innerWidth'), 'Game bar must fit the window'
                page.screenshot(path=str(OUT / f'{label}-plate.png'))

                page.get_by_role('tab', name='CHESS', exact=False).click()
                if args.cdp:
                    wait_leds(lambda batches: bool(batches) and batches[-1] == [])
                page.get_by_role('tab', name='DUSTLINE', exact=False).click()
                frame = game()
                assert frame.evaluate('localStorage.getItem("bridge-check")') == 'persisted'
                assert not errors, errors
                print(json.dumps({'mode': label, 'checks': 'sandbox boot, Web Audio, keyboard focus/pause, host storage and remount, source rejection, parent and in-frame fullscreen, game plate bounds' + (', synthetic Tauri fader boost/brake, knob alignment, Focus 1 vent and Control 1 repairs, native LED command delivery/validation and game-switch cleanup' if args.cdp else ''), 'frame': box, 'actualDevicesBeforeInjection':actual_devices, 'errors': errors}))
            finally:
                page.evaluate('([key,value])=>{if(value===null)localStorage.removeItem(key);else localStorage.setItem(key,value)}', [key, previous])
                page.evaluate('([key,value])=>{if(value===null)localStorage.removeItem(key);else localStorage.setItem(key,value)}', [midi_key, previous_midi])
                if args.cdp:
                    page.get_by_role('button', name='PLAY', exact=True).click()
                    page.evaluate("async()=>{const devices=await __TAURI__.core.invoke('midi_devices');await __TAURI__.event.emit('midi-devices',devices)}")
                else:
                    browser.close()
    finally:
        if server:
            server.shutdown()
            server.server_close()


if __name__ == '__main__':
    main()
