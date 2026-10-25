// Drawing for the synth displays (keys synth spec 7). Helpers are pure; draw* paint into a phosphor canvas.
import type { EnvParams, LfoParams, LfoShape } from './synth-types.ts';
import { txt } from './phosphor.ts';

export const freqToX = (hz: number, w: number) => (Math.log(hz / 20) / Math.log(1000)) * w;
export const xToFreq = (x: number, w: number) => 20 * Math.pow(1000, x / w);
/** dB to y on the FILTER grid (+24 dB at the top, −36 dB at the bottom). */
export const dbToY = (db: number, h: number, top = 24, bottom = -36) => ((top - db) / (top - bottom)) * h;

/** The display's time model: attack, decay, a sustain hold (a quarter of A + D + R, at least 50 ms), release. */
function envModel(e: EnvParams) {
  const a = Math.max(e.attack, 0.0005), d = Math.max(e.decay, 0), r = Math.max(e.release, 0.0005);
  const hold = Math.max(0.05, 0.25 * (a + d + r));
  return { a, d, r, hold, total: a + d + hold + r, s: Math.min(1, Math.max(0, e.sustain)), tau: a / Math.log(1.3 / 0.3) };
}

/** n points [x 0–1, level 0–1] of the ADSR curve, drawn with the engine's curves (spec 3.7). */
export function envPoints(e: EnvParams, n: number): [number, number][] {
  const m = envModel(e);
  const level = (t: number): number => {
    if (t <= m.a) return Math.min(1, 1.3 * (1 - Math.exp(-t / m.tau)));
    t -= m.a;
    if (t <= m.d) return m.d > 0 ? m.s + (1 - m.s) * Math.exp((Math.log(0.01) * t) / m.d) : m.s;
    t -= m.d;
    if (t <= m.hold) return m.s;
    t -= m.hold;
    return m.s * Math.exp((Math.log(0.01) * t) / m.r);
  };
  return Array.from({ length: n }, (_, i) => { const x = i / (n - 1); return [x, level(x * m.total)] as [number, number]; });
}

/** Where the live dot sits (x 0–1) for a monitor stage (1 attack … 4 release) and level; null when idle. */
export function envDotX(stage: number, level: number, e: EnvParams): number | null {
  const m = envModel(e);
  const at = (t: number) => Math.min(1, Math.max(0, t / m.total));
  switch (Math.round(stage)) {
    case 1: return at(-m.tau * Math.log(Math.max(1e-6, 1 - Math.min(level, 1.29) / 1.3)));
    case 2: return m.d > 0 && level > m.s ? at(m.a + (m.d * Math.log((level - m.s) / (1 - m.s))) / Math.log(0.01)) : at(m.a + m.d);
    case 3: return at(m.a + m.d + m.hold / 2);
    case 4: return m.s > 0 && level > 0 ? at(m.a + m.d + m.hold + (m.r * Math.log(level / m.s)) / Math.log(0.01)) : 1;
    default: return null;
  }
}

const STEPS = [0.6, -0.2, 0.9, -0.7, 0.1, -0.9, 0.4, -0.4];

/** −1…1 at `phase`; the random shapes draw a fixed example row of eight steps. */
export function lfoValue(shape: LfoShape, phase: number): number {
  const p = ((phase % 1) + 1) % 1;
  switch (shape) {
    case 'sine': return Math.sin(2 * Math.PI * p);
    case 'triangle': return p < 0.25 ? 4 * p : p < 0.75 ? 2 - 4 * p : 4 * p - 4;
    case 'sawUp': return 2 * p - 1;
    case 'sawDown': return 1 - 2 * p;
    case 'square': return p < 0.5 ? 1 : -1;
    case 'sampleHold': return STEPS[Math.floor(p * 8) % 8];
    default: {
      const i = Math.floor(p * 8) % 8, f = p * 8 - Math.floor(p * 8);
      return STEPS[i] + (STEPS[(i + 1) % 8] - STEPS[i]) * (0.5 - 0.5 * Math.cos(Math.PI * f));
    }
  }
}

/** One cycle from the start PHASE, with polarity applied. */
export function lfoPoints(l: LfoParams, n: number): number[] {
  return Array.from({ length: n }, (_, i) => {
    const v = lfoValue(l.shape, l.phase + i / n);
    return l.polarity === 'uni' ? 0.5 * (v + 1) : v;
  });
}

function trace(g: CanvasRenderingContext2D, points: [number, number][], color: string, width = 1.5) {
  g.strokeStyle = color; g.lineWidth = width; g.beginPath();
  points.forEach(([x,y], i) => { if (i) g.lineTo(x,y); else g.moveTo(x,y); });
  g.stroke();
}
function dot(g: CanvasRenderingContext2D, x: number, y: number) {
  g.fillStyle = '#ece3cc'; g.beginPath(); g.arc(x,y,2.5,0,Math.PI*2); g.fill();
}
export function drawWavetable(g: CanvasRenderingContext2D, W: number, H: number, frames: number[][] | null, pos: number, color: string) {
  if (!frames?.length) { txt(g,'STANDBY',W/2,H/2+5,color,'center'); return; }
  const selected = Math.round(Math.min(1,Math.max(0,pos))*(frames.length-1));
  const draw = (index: number, cream: boolean) => {
    const depth = index/Math.max(1,frames.length-1), frame = frames[index];
    g.globalAlpha = cream ? 1 : 0.5-0.35*depth;
    trace(g,frame.map((v,i)=>[i/Math.max(1,frame.length-1)*W*0.75+depth*W*0.25,H*0.65-v*H*0.22-depth*H*0.35]),cream?'#ece3cc':color);
  };
  const count=Math.min(24,frames.length);
  for(let i=count-1;i>=0;i--) draw(Math.round(i/Math.max(1,count-1)*(frames.length-1)),false);
  draw(selected,true);g.globalAlpha=1;
  const depth=selected/Math.max(1,frames.length-1),frame=frames[selected];
  dot(g,W*.375+depth*W*.25,H*.65-(frame[Math.floor(frame.length/2)]??0)*H*.22-depth*H*.35);
}
export function drawFilter(g: CanvasRenderingContext2D, W: number, H: number, curve: number[] | null, spectrum: ArrayLike<number>, cutoffHz: number, color: string) {
  g.strokeStyle='rgba(65,200,255,.22)';g.lineWidth=1;
  for(const db of [12,0,-12,-24]){g.beginPath();g.moveTo(0,dbToY(db,H));g.lineTo(W,dbToY(db,H));g.stroke();}
  for(const [hz,label] of [[100,'100'],[1000,'1k'],[10000,'10k']] as const) txt(g,label,freqToX(hz,W),H-2,'#7b8796','center');
  if(!curve?.length){txt(g,'STANDBY',W/2,H/2+5,color,'center');return;}
  g.fillStyle='rgba(65,200,255,.25)';g.beginPath();g.moveTo(0,H);
  for(let i=0;i<spectrum.length;i++)g.lineTo(i/Math.max(1,spectrum.length-1)*W,H-spectrum[i]*H);
  g.lineTo(W,H);g.closePath();g.fill();
  trace(g,curve.map((db,i)=>[i/Math.max(1,curve.length-1)*W,dbToY(db,H)]),color,2);
  g.strokeStyle=color;g.setLineDash([3,3]);g.beginPath();const x=freqToX(Math.min(20000,Math.max(20,cutoffHz)),W);g.moveTo(x,0);g.lineTo(x,H);g.stroke();g.setLineDash([]);
}
export function drawEnv(g: CanvasRenderingContext2D, W: number, H: number, e: EnvParams, monitor: [number,number] | null, color: string) {
  const points: [number,number][]=envPoints(e,96).map(([x,y])=>[x*W,(1-y)*(H-4)+2]);
  trace(g,points,color);g.lineTo(W,H);g.lineTo(0,H);g.closePath();g.globalAlpha=.12;g.fillStyle=color;g.fill();g.globalAlpha=1;
  if(monitor){const x=envDotX(monitor[0],monitor[1],e);if(x!==null)dot(g,x*W,(1-monitor[1])*(H-4)+2);}
}
export function drawLfo(g: CanvasRenderingContext2D, W: number, H: number, l: LfoParams, phase: number | null, color: string) {
  const y=(v:number)=>(1-(l.polarity==='uni'?v:(v+1)/2))*(H-4)+2;
  trace(g,lfoPoints(l,96).map((v,i)=>[i/95*W,y(v)]),color);
  if(phase!==null){const x=((phase-l.phase)%1+1)%1,v=lfoValue(l.shape,phase);dot(g,x*W,y(l.polarity==='uni'?(v+1)/2:v));}
}
export function drawScope(g: CanvasRenderingContext2D, W: number, H: number, samples: number[] | null) {
  if(samples?.length)trace(g,samples.map((v,i)=>[i/Math.max(1,samples.length-1)*W,H/2-v*H*.45]),'#7dffb0');
  else{trace(g,[[0,H/2],[W,H/2]],'#7dffb0');txt(g,'STANDBY',W/2,H/2+5,'#7dffb0','center');}
}
