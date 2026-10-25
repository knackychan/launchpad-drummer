// Shared phosphor canvas helpers, moved unchanged from Telemetry.svelte.
export type Lo = HTMLCanvasElement & { _labels?: [string, number, number, string][] };
export const PIX = 2;

export const F3 = (() => {
  const m: Record<string, number[]> = {};
  const d: Record<string, string> = {
    '0': '111101101101111', '1': '010110010010111', '2': '111001111100111', '3': '111001111001111', '4': '101101111001001',
    '5': '111100111001111', '6': '111100111101111', '7': '111001010010010', '8': '111101111101111', '9': '111101111001111',
    A: '010101111101101', B: '110101110101110', C: '011100100100011', D: '110101101101110', E: '111100110100111',
    F: '111100110100100', G: '011100101101011', H: '101101111101101', I: '111010010010111', J: '001001001101010',
    K: '101101110101101', L: '100100100100111', M: '101111111101101', N: '110101101101101', O: '010101101101010',
    P: '110101110100100', Q: '010101101111011', R: '110101110101101', S: '011100010001110', T: '111010010010010',
    U: '101101101101111', V: '101101101101010', W: '101101111111101', X: '101101010101101', Y: '101101010010010',
    Z: '111001010100111', ' ': '000000000000000', '-': '000000111000000', '.': '000000000000010', '·': '000000010000000',
    '/': '001001010100100', ':': '000010000010000', '+': '000010111010000', '%': '101001010100101',
  };
  for (const [k, v] of Object.entries(d)) m[k] = [0, 1, 2, 3, 4].map((r) => parseInt(v.substr(r * 3, 3), 2));
  return m;
})();

// 3×5 bitmap labels, queued and stamped after the crush so they stay legible.
export function txt(g: CanvasRenderingContext2D, s: string, x: number, y: number, col: string, al: 'left' | 'center' | 'right' = 'left') {
  s = s.toUpperCase().replace(/−/g, '-');
  const n = s.length * 4 - 1;
  const x0 = Math.round(x / PIX) - (al === 'center' ? Math.floor(n / 2) : al === 'right' ? n : 0), y0 = Math.round(y / PIX) - 5;
  const lo = g.canvas as Lo;
  (lo._labels ??= []).push([s, x0, y0, col]);
}
export function stamp(g: CanvasRenderingContext2D, [s, x0, y0, col]: [string, number, number, string]) {
  g.fillStyle = col;
  [...s].forEach((ch, i) => {
    const gl = F3[ch];
    if (gl) for (let r = 0; r < 5; r++) for (let b = 0; b < 3; b++) if ((gl[r] >> (2 - b)) & 1) g.fillRect(x0 + i * 4 + b, y0 + r, 1, 1);
  });
}
const los = new WeakMap<HTMLCanvasElement, Lo>();
export function fit(c: HTMLCanvasElement | undefined): [CanvasRenderingContext2D, number, number] | null {
  if (!c) return null;
  const d = devicePixelRatio || 1, w = c.clientWidth, h = c.clientHeight;
  if (!w || !h) return null;
  if (c.width !== Math.round(w * d) || c.height !== Math.round(h * d)) { c.width = Math.round(w * d); c.height = Math.round(h * d); }
  let lo = los.get(c);
  if (!lo) { lo = document.createElement('canvas') as Lo; los.set(c, lo); }
  const lw = Math.ceil(w / PIX), lh = Math.ceil(h / PIX);
  if (lo.width !== lw || lo.height !== lh) { lo.width = lw; lo.height = lh; }
  const g = lo.getContext('2d', { willReadFrequently: true })!;
  g.setTransform(1, 0, 0, 1, 0, 0); g.clearRect(0, 0, lw, lh); g.setTransform(1 / PIX, 0, 0, 1 / PIX, 0, 0);
  return [g, w, h];
}
export function present(c: HTMLCanvasElement | undefined) {
  const lo = c && los.get(c);
  if (!c || !lo || !c.width) return;
  const g = lo.getContext('2d', { willReadFrequently: true })!, im = g.getImageData(0, 0, lo.width, lo.height), a = im.data, W = lo.width;
  for (let i = 0; i < a.length; i += 4) {
    const al = a[i + 3];
    if (al < 20) { a[i + 3] = 0; continue; }
    const k = (((i >> 2) / W) | 0) & 1 ? 0.55 : 1;
    a[i] = Math.round(a[i] / 85) * 85 * k; a[i + 1] = Math.round(a[i + 1] / 85) * 85 * k; a[i + 2] = Math.round(a[i + 2] / 85) * 85 * k;
    a[i + 3] = al < 90 ? 110 : al < 180 ? 190 : 255;
  }
  g.putImageData(im, 0, 0);
  g.setTransform(1, 0, 0, 1, 0, 0);
  (lo._labels ?? []).forEach((l) => stamp(g, l));
  lo._labels = [];
  const o = c.getContext('2d')!, d = devicePixelRatio || 1, dw = lo.width * PIX * d, dh = lo.height * PIX * d;
  o.setTransform(1, 0, 0, 1, 0, 0); o.clearRect(0, 0, c.width, c.height); o.imageSmoothingEnabled = false;
  o.filter = `blur(${(2.2 * d).toFixed(1)}px)`; o.globalAlpha = 0.75; o.drawImage(lo, 0, 0, dw, dh);
  o.filter = 'none'; o.globalAlpha = 1; o.drawImage(lo, 0, 0, dw, dh);
}

