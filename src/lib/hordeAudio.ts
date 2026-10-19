// Horde sound. The app's CSP is default-src 'self' with a narrow connect-src, so fetch() + decodeAudioData is blocked;
// the mp3 files in /games/horde/sfx play through <audio> elements (media-src falls back to 'self'). The retro synth is the
// fallback when a file is missing, and a mode of its own.
export type SoundMode = 'sfx' | 'synth' | 'off';

const FILES = ['arm', 'base', 'bomb', 'clank', 'deny', 'freeze', 'hit', 'kick', 'kill', 'lose', 'miss', 'oops', 'rank', 'saved',
  'shot', 'snare', 'spawn', 'tick', 'wave'] as const;
type Sfx = typeof FILES[number];

const between = (a: number, b: number) => a + Math.random() * (b - a);

export function createHordeAudio(base = '/games/horde/sfx/') {
  let mode: SoundMode = 'sfx';
  let ac: AudioContext | null = null;
  let master: GainNode | null = null;
  const sources = new Map<Sfx, HTMLAudioElement>();
  const broken = new Set<Sfx>();

  function load() {
    if (sources.size || typeof Audio === 'undefined') return;
    for (const name of FILES) {
      const audio = new Audio(`${base}${name}.mp3`);
      audio.preload = 'auto';
      audio.addEventListener('error', () => broken.add(name));
      sources.set(name, audio);
    }
  }
  function context(): AudioContext | null {
    if (!ac && typeof AudioContext !== 'undefined') {
      try {
        ac = new AudioContext();
        master = ac.createGain(); master.gain.value = .52; master.connect(ac.destination);
      } catch { ac = null; }
    }
    if (ac?.state === 'suspended') void ac.resume();
    return ac;
  }
  function envelope(node: AudioNode, t0: number, attack: number, decay: number, peak: number) {
    const gain = ac!.createGain();
    gain.gain.setValueAtTime(.0001, t0);
    gain.gain.exponentialRampToValueAtTime(peak, t0 + attack);
    gain.gain.exponentialRampToValueAtTime(.0001, t0 + attack + decay);
    node.connect(gain); gain.connect(master!);
  }
  function tone(type: OscillatorType, f0: number, f1: number, attack: number, decay: number, peak = .8, delay = 0) {
    if (!context()) return;
    const t = ac!.currentTime + delay, osc = ac!.createOscillator();
    osc.type = type;
    osc.frequency.setValueAtTime(f0, t);
    osc.frequency.exponentialRampToValueAtTime(Math.max(20, f1), t + attack + decay);
    envelope(osc, t, attack, decay, peak);
    osc.start(t); osc.stop(t + attack + decay + .02);
  }
  function noise(duration: number, peak = .6, lowpass = 1200) {
    if (!context()) return;
    const t = ac!.currentTime, length = Math.floor(ac!.sampleRate * duration);
    const buffer = ac!.createBuffer(1, length, ac!.sampleRate), data = buffer.getChannelData(0);
    for (let i = 0; i < length; i++) data[i] = (Math.random() * 2 - 1) * (1 - i / length);
    const source = ac!.createBufferSource(), filter = ac!.createBiquadFilter();
    source.buffer = buffer; filter.type = 'lowpass'; filter.frequency.value = lowpass;
    source.connect(filter); envelope(filter, t, .005, duration, peak);
    source.start(t);
  }
  function play(name: Sfx, volume: number, rate: number, synth: () => void) {
    if (mode === 'off') return;
    const original = sources.get(name);
    if (mode === 'sfx' && original && !broken.has(name)) {
      const audio = original.cloneNode() as HTMLAudioElement;
      audio.volume = Math.min(1, volume * .6);
      audio.preservesPitch = false;
      audio.playbackRate = rate;
      audio.play().catch(() => { if (!ac) synth(); });
      return;
    }
    synth();
  }

  const ratio = (combo: number) => 2 ** (Math.min(combo, 18) / 28);
  return {
    init() { load(); context(); },
    setMode(next: SoundMode) { mode = next; if (next !== 'off') { load(); context(); } },
    shot() { play('shot', .72, between(.95, 1.06), () => tone('square', 1100, 260, .002, .06, .18)); },
    hit(combo = 0) { play('hit', .82, ratio(combo), () => { const f = 330 * 2 ** (Math.min(combo, 24) / 12); tone('square', f, f * 1.5, .003, .08, .28); noise(.06, .35, 4000); }); },
    kill(combo = 0) { play('kill', .88, ratio(combo), () => { const f = 440 * 2 ** (Math.min(combo, 24) / 12); tone('triangle', f, f * 2, .004, .14, .45); noise(.14, .45, 1800); }); },
    miss() { play('miss', .65, between(.95, 1.05), () => tone('sawtooth', 170, 70, .004, .12, .22)); },
    clank() { play('clank', .8, between(.95, 1.06), () => { tone('square', 1900, 1800, .001, .035, .18); tone('square', 1300, 1250, .001, .04, .12, .03); }); },
    base() { play('base', .95, 1, () => { noise(.35, .9, 500); tone('sine', 95, 35, .005, .35, .9); }); },
    bomb() { play('bomb', 1, 1, () => { noise(.6, 1, 700); tone('sine', 140, 28, .005, .6, 1); }); },
    freeze() { play('freeze', .9, 1, () => { tone('sine', 900, 2600, .02, .4, .35); tone('triangle', 1800, 3200, .02, .3, .2, .08); }); },
    spawn() { play('spawn', .55, between(.92, 1.08), () => tone('sine', 240, 170, .01, .12, .12)); },
    saved() { play('saved', .85, 1, () => { tone('triangle', 660, 660, .005, .08, .35); tone('triangle', 990, 990, .005, .12, .35, .09); }); },
    oops() { play('oops', .9, 1, () => tone('square', 400, 120, .01, .35, .35)); },
    rank() { play('rank', .85, 1, () => [523, 659, 784].forEach((f, i) => tone('square', f, f, .004, .07, .25, i * .06))); },
    wave() { play('wave', .85, 1, () => [392, 523, 659, 784].forEach((f, i) => tone('triangle', f, f, .005, .12, .4, i * .09))); },
    lose() { play('lose', .9, 1, () => [392, 330, 262, 196].forEach((f, i) => tone('sawtooth', f, f * .97, .01, .25, .35, i * .2))); },
    arm() { play('arm', .8, 1, () => tone('square', 300, 600, .01, .08, .3)); },
    kick() { play('kick', .85, 1, () => tone('sine', 160, 42, .002, .2, .9)); },
    snare() { play('snare', .75, 1, () => { noise(.12, .4, 3800); tone('triangle', 220, 180, .002, .06, .2); }); },
    tick() { play('tick', .5, 1, () => noise(.03, .12, 9000)); },
    deny() { play('deny', .75, 1, () => { tone('square', 200, 200, .003, .05, .25); tone('square', 160, 160, .003, .05, .25, .08); }); },
  };
}
export type HordeAudio = ReturnType<typeof createHordeAudio>;
