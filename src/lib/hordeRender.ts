// Canvas renderer for Horde: sea and sand, 2.5D keycaps with LED underglow, sprite enemies with walk frames, HUD, FX, post.
// Ported from prototypes/horde/index.html v1.2. It only reads game rules (HordeState) and view FX (Fx); it never changes either.
import { ENEMY, beatsPerMinute, cellsOf, gateFraction, multiplier, rankOf, stepProgress,
  type Enemy, type EnemyType, type HordeState } from './horde.ts';
import { H, W, type ArenaLayout } from './hordeLayout.ts';
import { LOOK, P, keySlot, type Fx, type HordeSettings } from './hordeFx.ts';

type Theme = { skyTop: string; skyBot: string; sun: string; seaDeep: string; seaMid: string; seaLit: string; foam: string; wet: string;
  sandTint: string; shadowCol: string; lx: number; ly: number };
// Time-of-day palettes shifting by wave: morning, high noon, boss crimson sunset, amber dusk, starlit night.
const THEMES: Theme[] = [
  { skyTop: '#1b4d6e', skyBot: '#69b7ce', sun: '#ffe885', seaDeep: '#115466', seaMid: '#1f7f8c', seaLit: '#38b6c5', foam: '#e9f7f3', wet: 'rgba(31,115,130,0.42)', sandTint: 'rgba(255,240,200,0)', shadowCol: 'rgba(25,20,15,0.28)', lx: -0.35, ly: 0.45 },
  { skyTop: '#145882', skyBot: '#80d0e0', sun: '#fff7c2', seaDeep: '#0e4c61', seaMid: '#228896', seaLit: '#45c4d4', foam: '#f0fcf9', wet: 'rgba(28,110,126,0.38)', sandTint: 'rgba(255,248,220,0.04)', shadowCol: 'rgba(20,18,12,0.30)', lx: -0.1, ly: 0.42 },
  { skyTop: '#361638', skyBot: '#e06b43', sun: '#ffbe42', seaDeep: '#28193d', seaMid: '#6b2d46', seaLit: '#ba4a4a', foam: '#ffd9c4', wet: 'rgba(92,38,54,0.45)', sandTint: 'rgba(240,120,70,0.10)', shadowCol: 'rgba(35,12,24,0.35)', lx: 0.45, ly: 0.52 },
  { skyTop: '#1c2242', skyBot: '#b86548', sun: '#ff9e44', seaDeep: '#142d47', seaMid: '#24566b', seaLit: '#418b9c', foam: '#e8eceb', wet: 'rgba(25,68,88,0.44)', sandTint: 'rgba(210,140,95,0.08)', shadowCol: 'rgba(20,16,28,0.33)', lx: 0.35, ly: 0.50 },
  { skyTop: '#080d1a', skyBot: '#1e3658', sun: '#d8ecff', seaDeep: '#092033', seaMid: '#144359', seaLit: '#256d85', foam: '#d0edf5', wet: 'rgba(14,48,68,0.48)', sandTint: 'rgba(40,85,140,0.12)', shadowCol: 'rgba(8,12,24,0.38)', lx: -0.25, ly: 0.48 },
];
const themeIndex = (wave: number) => {
  const w = Math.max(1, wave || 1);
  return w % 3 === 0 ? 2 : [0, 0, 1, 2, 3, 4][(w - 1) % 5 + 1] || 0;
};

const clamp = (v: number, a: number, b: number) => v < a ? a : v > b ? b : v;
const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
const sizeOf = (e: Enemy) => e.type === 'boss' ? 2 : 1;

const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
const CRUSH_LUTS = new Map<number, Uint8Array>();
function crushLut(bits: number): Uint8Array {
  let lut = CRUSH_LUTS.get(bits);
  if (lut) return lut;
  lut = new Uint8Array(16 * 256);
  const step = 255 / ((1 << bits) - 1);
  for (let b = 0; b < 16; b++) {
    const threshold = (BAYER[b] / 16 - .5) * step;
    for (let v = 0; v < 256; v++) lut[(b << 8) | v] = clamp(Math.round((v + threshold) / step) * step, 0, 255);
  }
  CRUSH_LUTS.set(bits, lut);
  return lut;
}

function roundRect(c: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  c.beginPath(); c.moveTo(x + r, y); c.arcTo(x + w, y, x + w, y + h, r); c.arcTo(x + w, y + h, x, y + h, r);
  c.arcTo(x, y + h, x, y, r); c.arcTo(x, y, x + w, y, r); c.closePath();
}

export function createRenderer(canvas: HTMLCanvasElement) {
  const ctx = canvas.getContext('2d')!;
  const layer = (width = W) => { const c = document.createElement('canvas'); c.width = width; c.height = H; return c; };
  let curVW = W;
  let sandCv = layer(curVW), boardCv = layer(curVW);
  let sandCtx = sandCv.getContext('2d')!, boardCtx = boardCv.getContext('2d')!;
  const bloomCv = document.createElement('canvas'); bloomCv.width = 160; bloomCv.height = 90;
  const bloomCtx = bloomCv.getContext('2d')!;
  let cacheKey = '';
  let skyGrad: CanvasGradient, seaGrad: CanvasGradient, wetGrad: CanvasGradient;
  // Per-frame inputs, set by draw().
  let S!: HordeState, F!: Fx, L!: ArenaLayout, cfg!: HordeSettings, theme!: Theme;
  // Which enemy owns or telegraphs each cell (zero allocation per frame).
  const owner: (Enemy | null)[] = new Array(120).fill(null), tele: (Enemy | null)[] = new Array(120).fill(null);

  // ---- text and shape helpers ----
  function text(t: string, x: number, y: number, size = 8, color: string = P.cream, align: CanvasTextAlign = 'left', outline = true) {
    const rx = Math.round(x), ry = Math.round(y);
    ctx.font = `900 ${size}px "Consolas", "Lucida Console", "Courier New", monospace`;
    ctx.textAlign = align; ctx.textBaseline = 'middle';
    if (outline) {
      ctx.lineJoin = 'round'; ctx.lineWidth = Math.max(2.2, size * .26); ctx.strokeStyle = P.ink;
      ctx.strokeText(t, rx, ry + (size >= 14 ? 1 : .5)); ctx.strokeText(t, rx, ry);
    }
    ctx.fillStyle = color; ctx.fillText(t, rx, ry);
  }
  const rr = (x: number, y: number, w: number, h: number, r: number) => roundRect(ctx, x, y, w, h, r);
  function fillOut(fill: string, lw = 2) { ctx.fillStyle = fill; ctx.fill(); ctx.lineWidth = lw; ctx.strokeStyle = P.ink; ctx.stroke(); }
  function ell(x: number, y: number, rx: number, ry: number) { ctx.beginPath(); ctx.ellipse(x, y, rx, ry, 0, 0, Math.PI * 2); }

  // ---- static layers ----
  function drawKeycapTo(c: number, r: number, sc: CanvasRenderingContext2D, own: Enemy | null, telegraph: Enemy | null, pressTimer: number, frozen: boolean) {
    const isKeys = L.kind === 'keys';
    const showLabel = isKeys || cfg.aids === 'full';
    const arenaCx = L.arena.x + L.arena.w * .5;
    const maxDepth = isKeys ? 4.8 : 3.6;
    const R = L.rect(c, r);
    const weightDip = own ? ((own.moveT ?? 1) < .35 ? Math.sin((own.moveT ?? 1) / .35 * Math.PI) * .35 : .15) : 0;
    const pressFrac = clamp(pressTimer / .18 + weightDip, 0, 1);
    const depth = maxDepth * (1 - pressFrac * .78);
    const pressY = maxDepth - depth;
    const skewX = ((R.x + R.w * .5 - arenaCx) / (L.arena.w * .5)) * (isKeys ? 1.8 : 1.1);

    // 1. recessed socket well
    const wx = R.x + 2, wy = R.y + 2, ww = R.w - 4, wh = R.h - 3;
    roundRect(sc, wx, wy, ww, wh, 5);
    sc.fillStyle = isKeys ? '#3b2f1e' : '#7a6338'; sc.fill();

    let glowCol: string | null = null, sideCol = '#b89d68', topCol: string = P.cream, rimCol = 'rgba(255,250,230,0.75)';
    let isArmour = false, isWeak = false;
    if (own) {
      const look = LOOK[own.type];
      if (frozen) { glowCol = P.ice; sideCol = '#3b8da8'; topCol = '#d4f3ff'; rimCol = P.white; }
      else if (own.type === 'boss') {
        const [wc, wr] = cellsOf(own)[own.weak ?? 0];
        if (wc === c && wr === r) { isWeak = true; glowCol = P.pink; sideCol = '#b52b65'; topCol = '#ffd6e7'; rimCol = P.white; }
        else { isArmour = true; glowCol = P.red2; sideCol = '#4a2525'; topCol = '#8c5852'; rimCol = '#b87b74'; }
      } else {
        glowCol = look.c1; sideCol = look.c2;
        topCol = own.type === 'grunt' ? '#c8f5d6' : own.type === 'runner' ? '#fceaa6' : own.type === 'brute' ? '#ffd2ab' : own.type === 'splitter' ? '#e5d4fc' : '#e0f7fa';
        rimCol = P.white;
      }
    } else if (telegraph) { glowCol = LOOK[telegraph.type].c1; topCol = '#f7ebd2'; }
    else if (pressTimer > 0) { sideCol = '#9c8252'; topCol = '#e3cfa3'; }

    // 2. under-key LED glow
    if (glowCol) {
      sc.save();
      sc.globalAlpha = own ? .72 : .25 + .18 * Math.sin(S.time * 14);
      sc.fillStyle = glowCol;
      roundRect(sc, wx - .5, wy - .5, ww + 1, wh + 1, 5.5); sc.fill();
      sc.restore();
    }
    // 3. extruded side wall
    const kx = R.x + 3.5 + skewX * .35, ky = R.y + 2 + pressY, kw = R.w - 7, kh = R.h - 4 - maxDepth;
    roundRect(sc, R.x + 3, ky + 1, R.w - 6, kh + depth, 4.5);
    sc.fillStyle = sideCol; sc.fill();
    sc.lineWidth = isKeys ? 1.5 : 1.1; sc.strokeStyle = isKeys ? '#261e12' : 'rgba(20,20,20,0.55)'; sc.stroke();
    // 4. top face, specular bevel, dish
    roundRect(sc, kx, ky, kw, kh, 4);
    sc.fillStyle = topCol; sc.fill();
    sc.strokeStyle = rimCol; sc.lineWidth = 1;
    sc.beginPath(); sc.moveTo(kx + 3, ky + .8); sc.lineTo(kx + kw - 3, ky + .8); sc.stroke();
    if (isKeys) {
      roundRect(sc, kx + 3, ky + 2.5, kw - 6, kh - 5, 3);
      sc.fillStyle = own ? 'rgba(255,255,255,0.22)' : 'rgba(184,157,104,0.18)'; sc.fill();
    }
    // 5. legend
    if (showLabel) {
      sc.font = `900 ${isKeys ? 14 : 8.5}px "Segoe UI", system-ui, sans-serif`;
      sc.textAlign = 'center'; sc.textBaseline = 'middle';
      sc.fillStyle = own ? P.ink : isKeys ? '#9c8150' : '#bda26e';
      sc.fillText(L.label(c, r), kx + kw * .5, ky + kh * .5 + .5);
    }
    // 6. lit target brackets, or armour bolts
    if (own) {
      const bx = kx + 1.5, by = ky + 1.5, bw = kw - 3, bh = kh - 3;
      if (isArmour) {
        sc.fillStyle = '#331818';
        sc.fillRect(bx + 1, by + 1, 2.5, 2.5); sc.fillRect(bx + bw - 3.5, by + 1, 2.5, 2.5);
        sc.fillRect(bx + 1, by + bh - 3.5, 2.5, 2.5); sc.fillRect(bx + bw - 3.5, by + bh - 3.5, 2.5, 2.5);
      } else {
        sc.strokeStyle = isWeak ? P.pink : own.type === 'survivor' ? P.teal : LOOK[own.type].c1;
        sc.lineWidth = 2;
        const cl = isKeys ? 6 : 4.5;
        sc.beginPath();
        sc.moveTo(bx, by + cl); sc.lineTo(bx, by); sc.lineTo(bx + cl, by);
        sc.moveTo(bx + bw - cl, by); sc.lineTo(bx + bw, by); sc.lineTo(bx + bw, by + cl);
        sc.moveTo(bx, by + bh - cl); sc.lineTo(bx, by + bh); sc.lineTo(bx + cl, by + bh);
        sc.moveTo(bx + bw - cl, by + bh); sc.lineTo(bx + bw, by + bh); sc.lineTo(bx + bw, by + bh - cl);
        sc.stroke();
      }
    }
  }

  function drawBeachWingDecor(sc: CanvasRenderingContext2D, VW: number, wingW: number, shY: number, _bh: number) {
    if (L.kind !== 'pad') return;
    const leftLimit = L.arena.x - 110;
    const rightStart = L.arena.x + L.arena.w + 90;

    const drawPalm = (px: number, py: number, leanRight = true, scale = 1) => {
      sc.save(); sc.translate(px, py); sc.scale(scale, scale);
      sc.fillStyle = 'rgba(20,16,10,0.28)'; ell(leanRight ? 12 : -12, 42, 18, 7); sc.fill();
      sc.strokeStyle = P.ink; sc.lineWidth = 1.6;
      for (let s = 0; s < 5; s++) {
        const segX = (leanRight ? 1 : -1) * (s * 3 + Math.sin(s * .8) * 4);
        const segY = 36 - s * 8;
        roundRect(sc, segX - 3.5, segY, 7, 9, 2);
        sc.fillStyle = s % 2 === 0 ? '#8c683e' : '#a8814d'; sc.fill(); sc.stroke();
      }
      const topX = (leanRight ? 1 : -1) * 16, topY = -3;
      sc.fillStyle = '#5c4125';
      ell(topX - 3, topY + 4, 3, 3); sc.fill();
      ell(topX + 3, topY + 4, 3, 3); sc.fill();
      ell(topX, topY + 7, 3, 3); sc.fill();
      const fronds = [[-24, -14], [-14, -22], [4, -26], [22, -22], [30, -12], [18, 0], [-18, 2]];
      for (const [fx, fy] of fronds) {
        sc.beginPath(); sc.moveTo(topX, topY);
        const midX = (topX + fx) * .5 + (fx > 0 ? 4 : -4), midY = topY - 8;
        sc.quadraticCurveTo(midX, midY, topX + fx, topY + fy);
        sc.lineWidth = 3.2; sc.strokeStyle = '#2c8a52'; sc.stroke();
        sc.lineWidth = 1.6; sc.strokeStyle = '#5fd38a'; sc.stroke();
      }
      sc.restore();
    };

    const drawWatchtower = (wx: number, wy: number) => {
      sc.save(); sc.translate(wx, wy);
      sc.fillStyle = 'rgba(20,16,10,0.3)'; roundRect(sc, -16, 36, 32, 8, 4); sc.fill();
      sc.strokeStyle = '#4a3720'; sc.lineWidth = 2;
      sc.beginPath();
      sc.moveTo(-10, 36); sc.lineTo(-7, 12);
      sc.moveTo(10, 36); sc.lineTo(7, 12);
      sc.moveTo(-10, 24); sc.lineTo(10, 24);
      sc.moveTo(-9, 12); sc.lineTo(9, 28);
      sc.moveTo(9, 12); sc.lineTo(-9, 28);
      sc.stroke();
      roundRect(sc, -14, 8, 28, 5, 2); sc.fillStyle = '#8c683e'; sc.fill(); sc.strokeStyle = P.ink; sc.stroke();
      sc.strokeStyle = '#6f532f'; sc.lineWidth = 1.2; sc.strokeRect(-12, 1, 24, 7);
      sc.beginPath(); sc.moveTo(-16, 2); sc.lineTo(0, -7); sc.lineTo(16, 2); sc.closePath();
      sc.fillStyle = '#e06b43'; sc.fill(); sc.strokeStyle = P.ink; sc.lineWidth = 1.5; sc.stroke();
      sc.strokeStyle = P.ink; sc.lineWidth = 1.5;
      sc.beginPath(); sc.moveTo(0, -7); sc.lineTo(0, -15); sc.stroke();
      ell(0, -16, 3, 3); sc.fillStyle = P.amber; sc.fill(); sc.stroke();
      sc.restore();
    };

    const drawAmmoCache = (cx: number, cy: number) => {
      sc.save(); sc.translate(cx, cy);
      sc.fillStyle = 'rgba(20,16,10,0.32)'; roundRect(sc, -18, 14, 40, 8, 4); sc.fill();
      roundRect(sc, -16, -2, 22, 16, 2);
      sc.fillStyle = '#6f532f'; sc.fill(); sc.lineWidth = 1.5; sc.strokeStyle = P.ink; sc.stroke();
      sc.fillStyle = '#9c7b4f'; sc.fillRect(-14, 2, 18, 8);
      sc.font = '900 5px monospace'; sc.fillStyle = '#22190f'; sc.textAlign = 'center'; sc.fillText('BOMB', -5, 7);
      roundRect(sc, -12, -14, 16, 12, 2);
      sc.fillStyle = '#806037'; sc.fill(); sc.strokeStyle = P.ink; sc.stroke();
      sc.fillStyle = '#22252a';
      ell(11, 10, 4, 4); sc.fill(); sc.strokeStyle = P.ink; sc.stroke();
      ell(18, 10, 4, 4); sc.fill(); sc.stroke();
      ell(14.5, 4, 4, 4); sc.fill(); sc.stroke();
      sc.fillStyle = 'rgba(255,255,255,0.4)';
      sc.fillRect(10, 8, 1.5, 1.5); sc.fillRect(17, 8, 1.5, 1.5); sc.fillRect(13.5, 2.5, 1.5, 1.5);
      roundRect(sc, -24, -4, 7, 18, 2);
      sc.fillStyle = '#2a4454'; sc.fill(); sc.strokeStyle = P.ink; sc.stroke();
      roundRect(sc, -23, 0, 5, 10, 1); sc.fillStyle = P.ice; sc.fill();
      sc.restore();
    };

    const drawPier = (jx: number, jy: number) => {
      sc.save(); sc.translate(jx, jy);
      for (let p = 0; p < 4; p++) {
        const py = -p * 7;
        roundRect(sc, -14, py, 28, 5, 1);
        sc.fillStyle = p % 2 === 0 ? '#6f532f' : '#8c683e'; sc.fill();
        sc.strokeStyle = P.ink; sc.lineWidth = 1.2; sc.stroke();
      }
      sc.fillStyle = '#3a2b19';
      sc.fillRect(-12, -26, 3, 26); sc.fillRect(9, -26, 3, 26);
      sc.strokeStyle = P.foam; sc.lineWidth = 1.5;
      sc.beginPath(); sc.ellipse(0, -22, 14, 3, 0, 0, Math.PI * 2); sc.stroke();
      sc.restore();
    };

    const drawCat = (kx: number, ky: number) => {
      sc.save(); sc.translate(kx, ky);
      sc.fillStyle = 'rgba(20,16,10,0.24)'; ell(0, 5, 8, 4); sc.fill();
      ell(0, 0, 7, 5); sc.fillStyle = P.orange; sc.fill(); sc.strokeStyle = P.ink; sc.lineWidth = 1.2; sc.stroke();
      sc.beginPath(); sc.moveTo(-5, -3); sc.lineTo(-7, -7); sc.lineTo(-2, -5); sc.closePath(); sc.fillStyle = P.orange; sc.fill(); sc.stroke();
      sc.beginPath(); sc.moveTo(2, -5); sc.lineTo(7, -7); sc.lineTo(5, -3); sc.closePath(); sc.fill(); sc.stroke();
      sc.beginPath(); sc.arc(6, 1, 4, -Math.PI / 2, Math.PI / 2); sc.strokeStyle = P.orange2; sc.lineWidth = 2; sc.stroke();
      sc.font = '700 6px monospace'; sc.fillStyle = '#7a3b12'; sc.fillText('z', -1, 0);
      sc.restore();
    };

    if (leftLimit > 25) {
      drawPalm(Math.min(leftLimit - 20, Math.max(22, wingW * .35)), shY + 22, true, 1);
      if (leftLimit > 70) drawWatchtower(Math.max(30, leftLimit - 58), shY + 36);
      if (leftLimit > 50) drawCat(Math.max(16, leftLimit - 38), shY + 68);
    }

    if (VW - rightStart > 25) {
      drawAmmoCache(Math.max(rightStart + 24, VW - Math.max(34, wingW * .4)), shY + 54);
      if (VW - rightStart > 70) drawPier(Math.min(VW - 32, rightStart + 76), shY + 6);
      if (VW - rightStart > 50) drawPalm(Math.min(VW - 22, rightStart + 36), shY + 22, false, 0.9);
    }
  }

  function buildSandCache() {
    const sc = sandCtx;
    const VW = L.VW || W;
    sc.clearRect(0, 0, VW, H);
    const shY = L.sea.h, bh = H - shY, skyH = L.kind === 'keys' ? 22 : 12;
    skyGrad = ctx.createLinearGradient(0, 0, 0, skyH);
    skyGrad.addColorStop(0, theme.skyTop); skyGrad.addColorStop(1, theme.skyBot);
    seaGrad = ctx.createLinearGradient(0, skyH, 0, shY);
    seaGrad.addColorStop(0, theme.seaDeep); seaGrad.addColorStop(.55, theme.seaMid); seaGrad.addColorStop(1, theme.seaLit);
    wetGrad = ctx.createLinearGradient(0, shY - 2, 0, shY + 24);
    wetGrad.addColorStop(0, theme.wet); wetGrad.addColorStop(.65, 'rgba(65,50,28,0.22)'); wetGrad.addColorStop(1, 'rgba(65,50,28,0)');

    const bg = sc.createLinearGradient(0, shY, 0, H);
    bg.addColorStop(0, '#c7ab73'); bg.addColorStop(.14, '#d9c089'); bg.addColorStop(.55, '#e0c996'); bg.addColorStop(1, '#ccb17a');
    sc.fillStyle = bg; sc.fillRect(0, shY, VW, bh);
    for (let i = 0; i < 5; i++) {
      const dy = shY + 28 + i * 62;
      sc.fillStyle = i % 2 === 0 ? 'rgba(255,244,214,0.16)' : 'rgba(155,126,76,0.14)';
      sc.beginPath(); sc.moveTo(0, dy);
      for (let x = 0; x <= VW; x += 16) sc.lineTo(x, dy + Math.sin(x * .025 + i * 1.7) * 8 + Math.cos(x * .011 - i) * 4);
      sc.lineTo(VW, H); sc.lineTo(0, H); sc.closePath(); sc.fill();
    }
    let seed = 1337;
    const sRnd = () => { seed = (seed * 1664525 + 1013904223) >>> 0; return seed / 4294967296; };
    const tones = ['#b59860', '#c4a870', '#ebdca8', '#9e824c', '#f5e7b8'];
    const grainCount = Math.round(1800 * (VW / 640));
    for (let i = 0; i < grainCount; i++) {
      sc.fillStyle = tones[i % tones.length];
      sc.fillRect(Math.floor(sRnd() * VW), shY + Math.floor(sRnd() * bh), i % 7 === 0 ? 2 : 1, 1);
    }
    const pebbleCount = Math.round(45 * (VW / 640));
    for (let i = 0; i < pebbleCount; i++) {
      const side = i % 2 === 0 ? sRnd() * (L.arena.x - 20) : (L.arena.x + L.arena.w + 20) + sRnd() * (VW - (L.arena.x + L.arena.w + 20));
      const x = L.kind === 'pad' ? Math.floor(side) : Math.floor(sRnd() * VW), y = shY + 14 + sRnd() * (bh - 24);
      if (x > L.arena.x - 12 && x < L.arena.x + L.arena.w + 12 && y > L.arena.y - 8 && y < L.arena.y + L.arena.h + 12) continue;
      sc.fillStyle = 'rgba(80,62,35,0.28)'; sc.fillRect(x + 1, y + 1, 3, 2);
      sc.fillStyle = i % 3 === 0 ? '#e8d8b0' : '#a88e58'; sc.fillRect(x, y, 3, 2);
    }
    sc.fillStyle = theme.sandTint; sc.fillRect(0, shY, VW, bh);

    const wL = L.wingW || 0;
    const props: [number, number, string, 'star' | 'shell' | 'wood'][] = L.kind === 'pad'
      ? [
          [Math.max(16, wL * .3), 342, P.pink, 'star'],
          [Math.max(48, wL + 24), 345, P.teal, 'shell'],
          [VW - Math.max(22, wL * .35), 92, P.red, 'star'],
          [VW - Math.max(30, wL * .4), 338, P.orange, 'shell'],
          [Math.max(26, wL * .4), 165, '#d9b882', 'wood'],
          [VW - Math.max(26, wL * .45), 218, P.pink2, 'shell'],
          [L.arena.x - 30, 344, P.yellow, 'shell'],
          [L.arena.x + L.arena.w + 48, 342, P.pink, 'star'],
          ...(wL > 30 ? [
            [20, 260, P.teal, 'shell'],
            [VW - 20, 160, P.pink, 'star'],
            [wL * .6, 320, '#d9b882', 'wood']
          ] as [number, number, string, 'star' | 'shell' | 'wood'][] : [])
        ]
      : [
          [16, 132, P.pink, 'star'], [VW - 16, 176, P.red, 'star'],
          [20, 238, P.teal, 'shell'], [VW - 22, 262, P.orange, 'shell']
        ];
    for (const [x, y, col, kind] of props) {
      sc.save(); sc.translate(x, y);
      sc.fillStyle = 'rgba(20,16,10,0.32)'; sc.beginPath(); sc.ellipse(1.5, 2.5, 7, 5, 0, 0, Math.PI * 2); sc.fill();
      if (kind === 'star') {
        sc.beginPath();
        for (let i = 0; i < 10; i++) { const a = i * Math.PI / 5 - Math.PI / 2, rad = i % 2 ? 3.2 : 7.5; sc.lineTo(Math.cos(a) * rad, Math.sin(a) * rad); }
        sc.closePath(); sc.fillStyle = col; sc.fill(); sc.lineWidth = 1.5; sc.strokeStyle = P.ink; sc.stroke();
        sc.fillStyle = 'rgba(255,255,255,0.45)'; sc.fillRect(-1, -1, 2, 2);
      } else if (kind === 'shell') {
        sc.beginPath(); sc.arc(0, 1, 6, Math.PI, 0); sc.lineTo(4, 6); sc.lineTo(-4, 6); sc.closePath();
        sc.fillStyle = col; sc.fill(); sc.lineWidth = 1.5; sc.strokeStyle = P.ink; sc.stroke();
        sc.strokeStyle = 'rgba(20,20,20,0.45)'; sc.lineWidth = 1;
        for (let s = -1; s <= 1; s++) { sc.beginPath(); sc.moveTo(0, 5); sc.lineTo(s * 3, -3); sc.stroke(); }
      } else {
        sc.rotate(-.25); roundRect(sc, -11, -3, 22, 6, 2);
        sc.fillStyle = '#8c683e'; sc.fill(); sc.lineWidth = 1.5; sc.strokeStyle = P.ink; sc.stroke();
        sc.fillStyle = '#ab8454'; sc.fillRect(-8, -1.5, 16, 2);
      }
      sc.restore();
    }

    drawBeachWingDecor(sc, VW, wL, shY, bh);

    // board chassis and every idle keycap, blitted once per frame
    const bc = boardCtx;
    bc.clearRect(0, 0, VW, H);
    if (L.kind === 'pad') {
      const a = L.arena;
      bc.fillStyle = 'rgba(20,15,8,0.36)'; roundRect(bc, a.x - 9, a.y + 3, a.w + 20, a.h + 14, 8); bc.fill();
      roundRect(bc, a.x - 8, a.y - 3, a.w + 16, a.h + 12, 7);
      bc.fillStyle = '#8a703f'; bc.fill(); bc.lineWidth = 2; bc.strokeStyle = P.ink; bc.stroke();
      roundRect(bc, a.x - 6, a.y - 2, a.w + 12, a.h + 7, 6);
      bc.fillStyle = '#dec995'; bc.fill();
    } else {
      for (let r = L.rows; r >= 1; r--) {
        const R1 = L.rect(1, r), R2 = L.rect(L.cols, r);
        bc.fillStyle = 'rgba(20,15,8,0.30)'; roundRect(bc, R1.x - 3, R1.y + 4, (R2.x + R2.w) - R1.x + 8, R1.h + 4, 7); bc.fill();
      }
    }
    for (let r = L.rows; r >= 1; r--) for (let c = 1; c <= L.cols; c++) drawKeycapTo(c, r, bc, null, null, 0, false);
  }

  function drawSea() {
    const VW = L.VW || W;
    const s = L.sea, low = cfg.quality === 'low';
    const skyH = L.kind === 'keys' ? 22 : 12;
    ctx.fillStyle = skyGrad; ctx.fillRect(0, 0, VW, skyH);
    const sunX = VW * .5 + (L.kind === 'pad' ? -195 : 220);
    ctx.fillStyle = theme.sun; ctx.globalAlpha = .85;
    ell(sunX, skyH - 2, L.kind === 'keys' ? 11 : 7, L.kind === 'keys' ? 11 : 7); ctx.fill();
    ctx.globalAlpha = 1;
    ctx.fillStyle = theme.seaDeep;
    ctx.beginPath();
    for (let x = 18; x < VW; x += 130) {
      ctx.moveTo(x, skyH); ctx.lineTo(x + 20, skyH - 6); ctx.lineTo(x + 34, skyH - 3); ctx.lineTo(x + 50, skyH - 8); ctx.lineTo(x + 74, skyH);
    }
    ctx.closePath(); ctx.fill();
    ctx.fillStyle = seaGrad; ctx.fillRect(0, skyH, VW, s.h - skyH);
    ctx.fillStyle = theme.sun; ctx.globalAlpha = .22;
    for (let y = skyH + 2; y < s.h - 6; y += 4) {
      const sw = 10 + (y - skyH) * .55 + Math.sin(S.time * 3 + y * .4) * 5;
      ctx.fillRect(sunX - sw / 2, y, sw, 2);
    }
    ctx.globalAlpha = 1;
    const stepY = low ? 10 : 6;
    for (let y = skyH + 3; y < s.h - 4; y += stepY) {
      const depth = (y - skyH) / Math.max(1, s.h - skyH);
      ctx.fillStyle = depth > .55 ? theme.foam : theme.seaLit;
      ctx.globalAlpha = .28 + depth * .42;
      const speed = 1.1 + depth * 1.4;
      for (let x = -24; x < VW + 24; x += 44) {
        const o = Math.sin(S.time * speed + y * .25 + x * .04) * (5 + depth * 4);
        ctx.fillRect(x + o + ((y / stepY | 0) % 2) * 21, y, 16 + depth * 12, depth > .7 ? 2.5 : 1.8);
      }
    }
    ctx.globalAlpha = 1;
  }

  function drawBeach() {
    ctx.drawImage(sandCv, 0, 0);
    const VW = L.VW || W;
    const s = L.sea, time = S.time;
    const tide = Math.sin(time * 1.6) * 3.5 + Math.sin(time * .7) * 2;
    const wetDepth = 16 + tide * 1.4;
    ctx.fillStyle = wetGrad; ctx.fillRect(0, s.h - 2, VW, wetDepth + 4);
    ctx.strokeStyle = 'rgba(233,247,243,0.28)'; ctx.lineWidth = 1.5;
    ctx.beginPath();
    for (let x = 0; x <= VW; x += 10) {
      const wy = s.h + 7 + Math.sin(time * 1.6 - .8 + x * .06) * 2.5;
      if (x === 0) ctx.moveTo(x, wy); else ctx.lineTo(x, wy);
    }
    ctx.stroke();
    ctx.fillStyle = 'rgba(31,127,140,0.45)';
    ctx.beginPath(); ctx.moveTo(0, s.h - 6);
    for (let x = 0; x <= VW; x += 8) ctx.lineTo(x, s.h + 2 + tide * .7 + Math.sin(time * 2.1 + x * .07) * 2.8);
    ctx.lineTo(VW, s.h - 6); ctx.closePath(); ctx.fill();
    ctx.fillStyle = theme.foam;
    ctx.beginPath(); ctx.moveTo(0, s.h - 2);
    for (let x = 0; x <= VW; x += 8) ctx.lineTo(x, s.h - 4 + tide * .5 + Math.sin(time * 2.2 + x * .07) * 2.8);
    for (let x = VW; x >= 0; x -= 8) ctx.lineTo(x, s.h + 1.5 + tide * .65 + Math.cos(time * 1.9 + x * .09) * 2.2);
    ctx.closePath(); ctx.fill();

    ctx.drawImage(boardCv, 0, 0);

    owner.fill(null); tele.fill(null);
    const showTele = cfg.aids !== 'low';
    const frozen = S.freezeIn > 0;
    for (const e of S.enemies) {
      const size = sizeOf(e);
      if (e.state === 'gate') {
        if (showTele) for (let d = 0; d < size; d++) tele[keySlot(e.column + d, L.rows)] = e;
        continue;
      }
      for (const [c, r] of cellsOf(e)) owner[keySlot(c, r)] = e;
      if (showTele && !frozen && e.row > 1 && stepProgress(S, e) > .55)
        for (let d = 0; d < size; d++) { const slot = keySlot(e.column + d, e.row - 1); if (!owner[slot]) tele[slot] = e; }
    }
    // only redraw keycaps that are occupied, telegraphed or pressed
    for (let r = L.rows; r >= 1; r--) for (let c = 1; c <= L.cols; c++) {
      const slot = keySlot(c, r), press = F.keyPress[slot];
      if (owner[slot] || tele[slot] || press > 0) drawKeycapTo(c, r, ctx, owner[slot], tele[slot], press, frozen);
    }
  }

  function drawBase() {
    const b = L.base, VW = L.VW || W;
    ctx.fillStyle = theme.shadowCol; rr(-4, b.y + 3, VW + 8, b.h + 20, 7); ctx.fill();
    rr(0, b.y, VW, b.h + 20, 6); fillOut('#165a66', 2);
    ctx.fillStyle = '#0f3f47'; ctx.fillRect(2, b.y + 10, VW - 4, b.h + 8);
    rr(3, b.y + 3, VW - 6, 9, 3); fillOut(P.teal, 1.5);
    ctx.fillStyle = 'rgba(255,255,255,0.28)'; ctx.fillRect(6, b.y + 4, VW - 12, 2);
    ctx.fillStyle = '#0a292e';
    for (let x = 14; x < VW - 10; x += 24) ctx.fillRect(x, b.y + 14, 2, 2);
    const hurtFlash = S.hurtT > 0 && cfg.motion !== 'reduce' && Math.floor(S.time * 20) % 2 === 0;
    const aStart = L.arena.x - 8, aEnd = L.arena.x + L.arena.w + 8;
    for (let x = 6; x < VW - 14; x += 18) {
      const isCentral = x >= aStart && x <= aEnd;
      ell(x + 8, b.y + 3.5, 9.5, 5); ctx.fillStyle = 'rgba(15,12,8,0.35)'; ctx.fill();
      ell(x + 8, b.y + 1.5, 9, 4.8); fillOut(hurtFlash ? P.red : (isCentral ? '#d9c38f' : '#b89f6d'), 1.5);
      ctx.fillStyle = hurtFlash ? '#ff8a80' : '#f3e3b8'; ctx.fillRect(x + 4, b.y - 1, 8, 1.5);
      ctx.strokeStyle = 'rgba(90,70,40,0.5)'; ctx.lineWidth = 1;
      ctx.beginPath(); ctx.moveTo(x + 8, b.y - 2); ctx.lineTo(x + 8, b.y + 5); ctx.stroke();
    }
    const t = L.turret, rec = F.muzzleT > 0 ? (F.muzzleT / .08) * 4.5 : 0;
    ell(t.x + theme.lx * 6, t.y + 4, 15, 12); ctx.fillStyle = theme.shadowCol; ctx.fill();
    ell(t.x, t.y + 1, 15, 12.5); fillOut('#454954', 2);
    ctx.save(); ctx.translate(t.x, t.y); ctx.rotate(F.turretAng);
    ctx.fillStyle = 'rgba(0,0,0,0.35)'; ctx.fillRect(2 - rec, -2, 24, 9);
    rr(4 - rec, -3.5, 22, 7, 2); fillOut(P.grey, 2);
    ctx.fillStyle = '#9ea3b0'; ctx.fillRect(6 - rec, -2, 18, 2);
    rr(0 - rec, -5, 11, 10, 2); fillOut('#50535c', 1.8);
    ctx.fillStyle = P.ink; ctx.fillRect(23 - rec, -4.5, 4, 9);
    if (F.muzzleT > 0) {
      ctx.fillStyle = P.orange;
      ctx.beginPath();
      ctx.moveTo(25, -3); ctx.lineTo(38, -8); ctx.lineTo(33, -2); ctx.lineTo(42, 0); ctx.lineTo(33, 2); ctx.lineTo(38, 8); ctx.lineTo(25, 3);
      ctx.closePath(); ctx.fill();
      ell(29, 0, 7.5, 5); fillOut(P.yellow, 1.4);
      ell(27, 0, 4, 2.8); ctx.fillStyle = P.white; ctx.fill();
    }
    ctx.restore();
    ell(t.x, t.y, 13, 11); fillOut(S.armed ? P.orange : P.red2, 2);
    ell(t.x, t.y - 1.5, 11, 8.5); ctx.fillStyle = S.armed ? P.yellow : P.red; ctx.fill();
    ell(t.x - 3.5, t.y - 4, 4.5, 3); ctx.fillStyle = S.armed ? P.white : P.pink; ctx.fill();
  }

  function enemyPos(e: Enemy) {
    const moveT = e.moveT ?? 1, ease = 1 - (1 - moveT) ** 3;
    const pc = e.pc ?? e.column, pr = e.pr ?? e.row;
    const a = L.rect(pc, pr), b = L.rect(e.column, e.row);
    let x = lerp(a.x, b.x, ease) + L.S / 2, y = lerp(a.y, b.y, ease) + L.S / 2;
    if (sizeOf(e) === 2) {
      const b2 = L.rect(e.column + 1, e.row + 1), a2 = L.rect(pc + 1, Math.min(pr + 1, L.rows + 1));
      x = (lerp(a.x, b.x, ease) + lerp(a2.x, b2.x, ease) + L.S) / 2; y = (lerp(a.y, b.y, ease) + lerp(a2.y, b2.y, ease) + L.S) / 2;
    }
    y -= Math.sin(Math.PI * moveT) * 6;
    return { x, y };
  }

  function reticle(x: number, y: number, rad: number, col: string, filled: boolean) {
    ctx.save(); ctx.translate(x, y); ctx.rotate(S.time * 2);
    if (filled) {
      ell(0, 0, rad * .5, rad * .5); ctx.fillStyle = Math.floor(S.time * 6) % 2 ? P.pink : P.white; ctx.fill(); ctx.lineWidth = 2; ctx.strokeStyle = P.ink; ctx.stroke();
    }
    ctx.lineWidth = 3; ctx.strokeStyle = P.ink; ctx.beginPath(); ctx.arc(0, 0, rad, 0, Math.PI * 2); ctx.stroke(); ctx.lineWidth = 2; ctx.strokeStyle = col; ctx.stroke();
    for (let i = 0; i < 4; i++) { ctx.rotate(Math.PI / 2); ctx.fillStyle = P.ink; ctx.fillRect(rad - 3, -2.5, 8, 5); ctx.fillStyle = col; ctx.fillRect(rad - 2, -1.5, 6, 3); }
    ctx.restore();
  }

  // ---- enemies: silhouettes, 3-frame walk, sea splash, squash, armour ----
  function drawEnemy(e: Enemy) {
    const { x, y } = enemyPos(e);
    const look = LOOK[e.type], size = sizeOf(e), Sz = L.S * size;
    const frozen = S.freezeIn > 0, reduce = cfg.motion === 'reduce';
    const wob = e.wob ?? 0, face = e.face ?? 0, hitT = e.hitT ?? 0;
    const animRate = e.type === 'runner' ? 14 : e.type === 'brute' ? 6.5 : 9.5;
    const frame = frozen ? 1 : Math.floor(S.time * animRate + wob * 2) % 3;
    const stride = frame === 0 ? -1 : frame === 2 ? 1 : 0;
    const bob = frozen ? 0 : (frame === 1 ? -1.6 : .8) + Math.sin(S.time * 8 + wob) * .8;
    const flash = hitT > 0 && (reduce ? true : Math.floor(hitT * 30) % 2 === 0);
    const c1 = flash ? P.white : frozen ? P.ice : look.c1;
    const c2 = flash ? P.cream : frozen ? P.teal : look.c2;
    const gate = e.state === 'gate';
    const sub = gate ? gateFraction(S, e) : 0;
    const k = Sz / 36;
    const sq = (e.squashT ?? 0) / .36;
    const hopStretch = (e.moveT ?? 1) < 1 ? Math.sin((e.moveT ?? 1) * Math.PI) * .14 : 0;
    const sx = 1 + sq * .36 - hopStretch * .5, sy = 1 - sq * .28 + hopStretch;

    ctx.save();
    ctx.translate(x, y + bob + sub * Sz * .28);
    if (gate) {
      ctx.strokeStyle = theme.foam; ctx.lineWidth = 2; ctx.globalAlpha = .75;
      ell(0, Sz * .18, Sz * (.42 + (1 - sub) * .15), Sz * .14); ctx.stroke();
      ctx.globalAlpha = 1;
    } else {
      ell(theme.lx * Sz * .16, Sz * .33, Sz * .34 * sx, Sz * .10); ctx.fillStyle = theme.shadowCol; ctx.fill();
    }
    ctx.save();
    ctx.scale(sx, sy);

    if (e.type === 'boss') {
      for (const s of [-1, 1]) {
        for (let l = 0; l < 3; l++) {
          const legPhase = ((frame + l) % 3) - 1, ly = (-2 + l * 5) * k;
          const lxOuter = (21 + l * 2) * k + legPhase * 2.2 * k, lyTip = (6 + l * 4.5 + Math.abs(legPhase) * 2) * k;
          ctx.beginPath(); ctx.moveTo(s * 14 * k, ly); ctx.lineTo(s * lxOuter, ly - 2 * k); ctx.lineTo(s * (lxOuter - 2 * k), lyTip);
          ctx.lineWidth = 3 * k * .65; ctx.lineCap = 'round'; ctx.strokeStyle = P.ink; ctx.stroke();
          ctx.lineWidth = 1.6 * k * .65; ctx.strokeStyle = c2; ctx.stroke();
        }
        const clawLift = stride * s * 2.2 * k;
        ctx.beginPath(); ctx.moveTo(s * 15 * k, -2 * k); ctx.lineTo(s * 25 * k, -10 * k + clawLift);
        ctx.lineWidth = 4.5; ctx.strokeStyle = P.ink; ctx.stroke(); ctx.lineWidth = 2.5; ctx.strokeStyle = c2; ctx.stroke();
        ctx.save(); ctx.translate(s * 27 * k, -12 * k + clawLift);
        ell(0, 0, 7.5 * k, 6 * k); fillOut(c1, 2);
        ctx.fillStyle = P.yellow; ctx.beginPath(); ctx.moveTo(0, -2 * k); ctx.lineTo(-s * 5 * k, -5 * k); ctx.lineTo(-s * 2 * k, 1 * k); ctx.closePath(); ctx.fill();
        ctx.restore();
      }
      ell(0, 0, 22 * k, 14.5 * k); fillOut(c2, 2.5);
      ell(0, -2.5 * k, 19.5 * k, 11.5 * k); ctx.fillStyle = c1; ctx.fill();
      const q = [[-8.5 * k, 4 * k], [8.5 * k, 4 * k], [-8.5 * k, -5 * k], [8.5 * k, -5 * k]];
      for (let qi = 0; qi < 4; qi++) {
        const [qx, qy] = q[qi];
        if (qi === (e.weak ?? 0)) {
          ell(qx, qy, 5.5 * k, 4 * k); fillOut('#4a0e2e', 1.5);
          const pulse = .7 + .3 * Math.sin(S.time * 14);
          ell(qx, qy, 4.2 * k * pulse, 3 * k * pulse); ctx.fillStyle = P.pink; ctx.fill();
          ell(qx, qy, 2.2 * k, 1.6 * k); ctx.fillStyle = P.white; ctx.fill();
        } else {
          rr(qx - 6 * k, qy - 3.5 * k, 12 * k, 7 * k, 2 * k); fillOut(flash ? P.cream : '#59252a', 1.5);
          ctx.fillStyle = '#8c3b42'; ctx.fillRect(qx - 4.5 * k, qy - 2 * k, 9 * k, 2 * k);
          ctx.fillStyle = P.cream2; ctx.fillRect(qx - 4.5 * k, qy + 1 * k, 1.5 * k, 1.5 * k); ctx.fillRect(qx + 3 * k, qy + 1 * k, 1.5 * k, 1.5 * k);
        }
      }
      for (let i = -1; i <= 1; i++) {
        ctx.beginPath(); ctx.moveTo(i * 5 * k - 2 * k, -12 * k); ctx.lineTo(i * 5 * k, -17 * k); ctx.lineTo(i * 5 * k + 2 * k, -12 * k);
        ctx.closePath(); fillOut(P.yellow, 1.5);
      }
      for (const s of [-1, 1]) {
        ctx.beginPath(); ctx.moveTo(s * 6 * k, -10 * k); ctx.lineTo(s * 8.5 * k, -20 * k); ctx.lineWidth = 3; ctx.strokeStyle = P.ink; ctx.stroke();
        ell(s * 8.5 * k, -21 * k, 4.2 * k, 4.2 * k); fillOut(P.white, 1.8);
        ell(s * 8.5 * k, -20 * k, 1.8 * k, 2.1 * k); ctx.fillStyle = P.ink; ctx.fill();
        ctx.beginPath(); ctx.moveTo(s * 12 * k, -24 * k); ctx.lineTo(s * 4.5 * k, -21.5 * k); ctx.lineWidth = 2.2; ctx.strokeStyle = P.ink; ctx.stroke();
      }
    } else if (e.type === 'survivor') {
      const lFoot = stride * 2.5 * k, rFoot = -stride * 2.5 * k;
      ell(-3.5 * k, 12 * k + lFoot, 2.6 * k, 1.8 * k); fillOut(P.ink, 1);
      ell(3.5 * k, 12 * k + rFoot, 2.6 * k, 1.8 * k); fillOut(P.ink, 1);
      rr(-6.5 * k, -4 * k, 13 * k, 16 * k, 4 * k); fillOut(flash ? P.cream : P.white, 2);
      ctx.fillStyle = P.teal; ctx.fillRect(-5.5 * k, -3 * k, 11 * k, 7 * k);
      ctx.fillStyle = P.yellow; ctx.fillRect(-5.5 * k, 1 * k, 11 * k, 1.8 * k);
      ell(0, 5 * k, 9.5 * k, 3.6 * k); fillOut(P.orange, 1.6);
      ctx.fillStyle = P.white; ctx.fillRect(-8.5 * k, 4 * k, 3 * k, 2 * k); ctx.fillRect(5.5 * k, 4 * k, 3 * k, 2 * k);
      const wL = frame === 0 ? 5 * k : frame === 1 ? 1 * k : -3 * k, wR = frame === 2 ? 5 * k : frame === 1 ? 1 * k : -3 * k;
      ctx.beginPath(); ctx.moveTo(-6 * k, -1 * k); ctx.lineTo(-13 * k, -8 * k - wL); ctx.moveTo(6 * k, -1 * k); ctx.lineTo(13 * k, -8 * k - wR);
      ctx.lineWidth = 2.4; ctx.lineCap = 'round'; ctx.strokeStyle = P.ink; ctx.stroke();
      ell(0, -9.5 * k, 6.2 * k, 6.2 * k); fillOut('#f5cba7', 2);
      ell(0, -14.5 * k, 5.5 * k, 2.4 * k); fillOut(P.white, 1.5);
      ctx.fillStyle = P.teal; ctx.fillRect(-4.5 * k, -13.5 * k, 9 * k, 1.6 * k);
      ctx.fillStyle = P.ink; ctx.fillRect(-3.2 * k, -10.5 * k, 1.8 * k, 2.2 * k); ctx.fillRect(1.6 * k, -10.5 * k, 1.8 * k, 2.2 * k);
      ell(0, -6.8 * k, 2 * k, 1.5 * k); ctx.fill();
      if (!gate) { rr(-13 * k, -25 * k, 26 * k, 8 * k, 3 * k); fillOut(P.teal2, 1.5); text('HELP', 0, -21 * k, 7, P.white, 'center', false); }
    } else if (e.type === 'grunt') {
      const lf = frame === 0 ? 2.5 * k : frame === 2 ? -1.5 * k : 0, rf = frame === 2 ? 2.5 * k : frame === 0 ? -1.5 * k : 0;
      ell(-6 * k, 9.5 * k + lf, 4.5 * k, 2.5 * k); fillOut(c2, 1.6);
      ell(6 * k, 9.5 * k + rf, 4.5 * k, 2.5 * k); fillOut(c2, 1.6);
      for (const s of [-1, 1]) {
        ctx.beginPath(); ctx.moveTo(s * 10 * k, -3 * k); ctx.lineTo(s * 15.5 * k, -6 * k + stride * s * k);
        ctx.lineTo(s * 14.5 * k, -1 * k); ctx.lineTo(s * 16 * k, 2 * k); ctx.lineTo(s * 10 * k, 3 * k); ctx.closePath(); fillOut(c2, 1.5);
      }
      ctx.beginPath(); ctx.moveTo(-4 * k, -9 * k); ctx.lineTo(-3 * k, -15 * k); ctx.lineTo(0, -11 * k);
      ctx.lineTo(0, -16.5 * k); ctx.lineTo(3 * k, -11 * k); ctx.lineTo(4 * k, -14.5 * k); ctx.lineTo(4 * k, -9 * k); ctx.closePath(); fillOut(c1, 1.5);
      ell(0, 0, 12 * k, 10.8 * k); fillOut(c2, 2);
      ell(0, -2 * k, 10.8 * k, 8.5 * k); ctx.fillStyle = c1; ctx.fill();
      ell(0, 3.5 * k, 7 * k, 4.5 * k); ctx.fillStyle = 'rgba(243,227,184,0.35)'; ctx.fill();
      for (const s of [-1, 1]) {
        ell(s * 4.5 * k, -3.5 * k, 3.4 * k, 3.8 * k); fillOut(P.white, 1.3);
        ell(s * 4.5 * k + face, -2.2 * k, 1.5 * k, 2 * k); ctx.fillStyle = P.ink; ctx.fill();
        ctx.beginPath(); ctx.moveTo(s * 8 * k, -8 * k); ctx.lineTo(s * 1.8 * k, -5.8 * k); ctx.lineWidth = 2; ctx.stroke();
      }
      ctx.fillStyle = P.ink; ctx.fillRect(-4 * k, 3.8 * k, 8 * k, 1.8 * k);
      ctx.fillStyle = P.white; ctx.fillRect(-3.5 * k, 2.2 * k, 1.8 * k, 2 * k); ctx.fillRect(1.8 * k, 2.2 * k, 1.8 * k, 2 * k);
    } else if (e.type === 'runner') {
      const lean = face * 2.5 * k;
      if (!frozen) {
        ctx.fillStyle = P.cream;
        for (let i = 0; i < 3; i++) ctx.fillRect((-8 + i * 8) * k, -14 * k - ((S.time * 65 + i * 5) % 9), 2, 5.5);
      }
      const legA = frame === 0 ? -4 * k : frame === 2 ? 4 * k : 0;
      ctx.beginPath(); ctx.moveTo(-3 * k, 6 * k); ctx.lineTo(-5 * k + legA, 12.5 * k); ctx.moveTo(3 * k, 6 * k); ctx.lineTo(5 * k - legA, 12.5 * k);
      ctx.lineWidth = 2.4; ctx.lineCap = 'round'; ctx.strokeStyle = P.ink; ctx.stroke();
      for (const s of [-1, 1]) {
        ctx.beginPath(); ctx.moveTo(s * 5 * k + lean, -5 * k); ctx.lineTo(s * 16 * k + lean, -15 * k + stride * s * 1.5 * k); ctx.lineTo(s * 11 * k + lean, -2 * k);
        ctx.closePath(); fillOut(c1, 1.6);
      }
      ctx.beginPath(); ctx.moveTo(lean, -11.5 * k); ctx.lineTo(11.5 * k + lean, -1.5 * k); ctx.lineTo(lean, 10 * k); ctx.lineTo(-11.5 * k + lean, -1.5 * k); ctx.closePath(); fillOut(c2, 2);
      ctx.beginPath(); ctx.moveTo(lean, -10 * k); ctx.lineTo(9.5 * k + lean, -2 * k); ctx.lineTo(lean, 7.5 * k); ctx.lineTo(-9.5 * k + lean, -2 * k); ctx.closePath(); ctx.fillStyle = c1; ctx.fill();
      for (const s of [-1, 1]) {
        ctx.beginPath(); ctx.moveTo(s * 1.5 * k + lean, -2.5 * k); ctx.lineTo(s * 7.5 * k + lean, -5.5 * k); ctx.lineTo(s * 6.5 * k + lean, -.5 * k); ctx.closePath(); fillOut(P.white, 1.2);
        ctx.fillStyle = P.ink; ctx.fillRect(s * 3.5 * k + lean - 1, -3.5 * k, 2, 2.5);
      }
    } else if (e.type === 'brute') {
      const b = 1.24;
      for (const s of [-1, 1]) {
        const fistY = (s === -1 ? stride : -stride) * 2.8 * k;
        ctx.beginPath(); ctx.moveTo(s * 9 * k * b, -8 * k); ctx.lineTo(s * 16 * k * b, -6 * k + fistY * .3); ctx.lineTo(s * 14 * k * b, 2 * k + fistY * .3); ctx.lineTo(s * 8 * k * b, 1 * k);
        ctx.closePath(); fillOut(c2, 1.8);
        rr(s * 12.5 * k * b - 3.5 * k, 1 * k + fistY, 7 * k, 6.5 * k, 2 * k); fillOut(c1, 1.8);
        ctx.beginPath(); ctx.moveTo(s * 6 * k * b, -9 * k); ctx.lineTo(s * 13.5 * k * b, -18.5 * k); ctx.lineTo(s * 2.5 * k * b, -11.5 * k); ctx.closePath(); fillOut(P.cream, 1.6);
      }
      rr(-12 * k * b, -10.5 * k * b, 24 * k * b, 21 * k * b, 6 * k); fillOut(c2, 2.2);
      rr(-10.8 * k * b, -9.5 * k * b, 21.6 * k * b, 16 * k * b, 5 * k); ctx.fillStyle = c1; ctx.fill();
      rr(-8.5 * k * b, 2.5 * k, 17 * k * b, 6.5 * k, 2 * k); fillOut('#5c3418', 1.6);
      for (const s of [-1, 1]) {
        ctx.beginPath(); ctx.moveTo(s * 6.5 * k, 4 * k); ctx.lineTo(s * 4.5 * k, -1.5 * k); ctx.lineTo(s * 2.5 * k, 4 * k); ctx.closePath(); fillOut(P.cream, 1.3);
        ell(s * 4.8 * k * b, -4 * k, 3.2 * k, 3.4 * k); fillOut(P.white, 1.3);
        ell(s * 4.8 * k * b + face, -3 * k, 1.6 * k, 1.8 * k); ctx.fillStyle = P.ink; ctx.fill();
        ctx.beginPath(); ctx.moveTo(s * 9 * k * b, -8.5 * k); ctx.lineTo(s * 1.5 * k, -5.5 * k); ctx.lineWidth = 2.5; ctx.stroke();
      }
    } else if (e.type === 'splitter') {
      const sep = (e.hp === 1 ? 5.6 : 4.2) * k + Math.abs(stride) * 1.1 * k;
      for (const s of [-1, 1]) {
        const lobeY = s * stride * 1.2 * k;
        ctx.beginPath(); ctx.moveTo(s * sep, -9 * k + lobeY); ctx.lineTo(s * (sep + 3 * k), -14.5 * k + lobeY); ctx.lineWidth = 2; ctx.strokeStyle = P.ink; ctx.stroke();
        ell(s * (sep + 3 * k), -15 * k + lobeY, 2.2 * k, 2.2 * k); fillOut(P.pink, 1.2);
        ell(s * (sep + 1 * k), 9.5 * k - lobeY, 4 * k, 2.2 * k); fillOut(c2, 1.5);
      }
      for (const s of [-1, 1]) {
        const lobeY = s * stride * 1 * k;
        ell(s * sep, lobeY, 8.8 * k, 10 * k); fillOut(c2, 2);
        ell(s * sep, lobeY - 1.5 * k, 7.6 * k, 8 * k); ctx.fillStyle = c1; ctx.fill();
      }
      const zig = (frame - 1) * 2.2 * k;
      ctx.beginPath(); ctx.moveTo(0, -10.5 * k); ctx.lineTo(-2.5 * k + zig, -4 * k); ctx.lineTo(2.5 * k - zig, 2 * k); ctx.lineTo(0, 9.5 * k);
      ctx.lineWidth = e.hp === 1 ? 3.2 : 2.2; ctx.strokeStyle = P.ink; ctx.stroke();
      ctx.lineWidth = e.hp === 1 ? 1.8 : 1.1; ctx.strokeStyle = e.hp === 1 ? P.white : P.pink; ctx.stroke();
      for (const s of [-1, 1]) {
        const lobeY = s * stride * 1 * k, lx = s * sep;
        for (const es of [-1, 1]) {
          ell(lx + es * 2.4 * k, lobeY - 2.5 * k, 2.1 * k, 2.5 * k); fillOut(P.white, 1);
          ctx.fillStyle = P.ink; ctx.fillRect(lx + es * 2.4 * k - .8 * k, lobeY - 2.2 * k, 1.6 * k, 1.8 * k);
        }
        ctx.fillStyle = P.ink; ctx.fillRect(lx - 2.5 * k, lobeY + 3.2 * k, 5 * k, 1.5 * k);
      }
    }
    if (frozen) {
      ctx.fillStyle = 'rgba(168,230,255,0.28)'; ctx.strokeStyle = P.white; ctx.lineWidth = 1.5;
      ctx.beginPath(); ctx.moveTo(-Sz * .34, -Sz * .28); ctx.lineTo(0, -Sz * .42); ctx.lineTo(Sz * .34, -Sz * .28);
      ctx.lineTo(Sz * .30, Sz * .32); ctx.lineTo(-Sz * .30, Sz * .32); ctx.closePath(); ctx.fill(); ctx.stroke();
    }
    ctx.restore();

    if (gate) {
      const spW = Sz * .38;
      ctx.fillStyle = theme.foam; ctx.beginPath(); ctx.moveTo(-spW, Sz * .18);
      for (let i = 0; i <= 6; i++) ctx.lineTo(-spW + (i / 6) * (spW * 2), Sz * .14 - (i % 2 ? 4.5 : 1) - Math.sin(S.time * 12 + i) * 2);
      ctx.lineTo(spW, Sz * .22); ctx.lineTo(-spW, Sz * .22); ctx.closePath(); fillOut(theme.foam, 1.4);
    }
    ctx.restore();
    if (gate) return;

    const max = e.max ?? ENEMY[e.type].hp;
    if (max > 1) {
      const w = Math.min(6, (Sz - 6) / max), x0 = x - max * w / 2, py = y - Sz * .48;
      ctx.fillStyle = P.ink; ctx.fillRect(x0 - 1, py - 1, max * w + 2, 5);
      for (let i = 0; i < max; i++) { ctx.fillStyle = i < e.hp ? (size === 2 ? P.red : P.orange) : '#3a3a44'; ctx.fillRect(x0 + i * w + .5, py, w - 1, 3); }
    }
    if (size === 2) {
      const [wc, wr] = cellsOf(e)[e.weak ?? 0], R = L.rect(wc, wr);
      reticle(R.x + R.w / 2, R.y + R.h / 2, 13, P.pink, true);
    } else if (e.row === 1 && e.type !== 'survivor') reticle(x, y, 15, P.pink, false);
  }

  // ---- FX ----
  function drawTelegraphs() {
    if (cfg.aids === 'low' || S.freezeIn > 0) return;
    for (const e of S.enemies) {
      if (e.state === 'gate' || e.row <= 1) continue;
      const p = stepProgress(S, e);
      if (p < .55) continue;
      ctx.globalAlpha = (p - .55) * 1.3; ctx.fillStyle = LOOK[e.type].c1;
      for (let d = 0; d < sizeOf(e); d++) {
        const Q = L.rect(e.column + d, e.row - 1), cx = Q.x + Q.w / 2, cy = Q.y + Q.h / 2 + Math.sin(S.time * 16) * 1.5;
        ctx.beginPath(); ctx.moveTo(cx - 6, cy - 3); ctx.lineTo(cx + 6, cy - 3); ctx.lineTo(cx, cy + 5); ctx.closePath();
        ctx.fill(); ctx.lineWidth = 1; ctx.strokeStyle = P.ink; ctx.stroke();
      }
      ctx.globalAlpha = 1;
    }
  }
  function drawCellFx() {
    for (const f of S.cellFx) {
      const R = L.rect(f.column, f.row), a = clamp(f.t / f.t0, 0, 1), cx = R.x + R.w / 2, cy = R.y + R.h / 2;
      if (f.kind === 'bomb') {
        ctx.save(); ctx.globalAlpha = a * .9; ctx.fillStyle = a > .6 ? P.yellow : P.orange;
        rr(R.x + 2, R.y + 2, R.w - 4, R.h - 4, 5); ctx.fill();
        ctx.strokeStyle = P.white; ctx.lineWidth = 2; ell(cx, cy, (R.w * .42) * (1.1 - a * .5), (R.h * .42) * (1.1 - a * .5)); ctx.stroke();
        ctx.restore();
      } else if (f.kind === 'miss') {
        ctx.save(); const prog = 1 - a; ctx.globalAlpha = a * .9; ctx.strokeStyle = P.red; ctx.lineWidth = 2;
        ell(cx, cy, 4 + prog * (R.w * .44), 3 + prog * (R.h * .38)); ctx.stroke();
        const s = 6 * (.7 + .3 * Math.sin(prog * Math.PI));
        ctx.strokeStyle = P.ink; ctx.lineWidth = 3.5;
        ctx.beginPath(); ctx.moveTo(cx - s, cy - s); ctx.lineTo(cx + s, cy + s); ctx.moveTo(cx + s, cy - s); ctx.lineTo(cx - s, cy + s); ctx.stroke();
        ctx.strokeStyle = P.red; ctx.lineWidth = 1.8; ctx.stroke();
        ctx.restore();
      } else {
        ctx.globalAlpha = a * .82; ctx.fillStyle = f.kind === 'hit' ? '#7fd6ff' : P.grey;
        rr(R.x + 3, R.y + 3, R.w - 6, R.h - 6, 5); ctx.fill(); ctx.globalAlpha = 1;
      }
    }
  }
  function drawDecals() {
    for (const d of F.decals) {
      ctx.globalAlpha = Math.min(1, d.t / 2) * .55; ctx.fillStyle = d.col;
      ctx.beginPath();
      for (let i = 0; i < 12; i++) { const a = i / 12 * Math.PI * 2, rad = (9 + ((Math.sin(d.seed + i * 2.3) + 1) * 4)) * d.s; ctx.lineTo(d.x + Math.cos(a) * rad, d.y + Math.sin(a) * rad * .6 + 6); }
      ctx.closePath(); ctx.fill();
    }
    ctx.globalAlpha = 1;
  }
  function drawFx() {
    for (const t of F.tracers) {
      const a = clamp(t.t / (t.t0 || .09), 0, 1);
      ctx.save();
      ctx.strokeStyle = P.ink; ctx.lineWidth = 5.5 * a; ctx.beginPath(); ctx.moveTo(L.turret.x, L.turret.y); ctx.lineTo(t.x, t.y); ctx.stroke();
      ctx.strokeStyle = S.armed ? P.orange : P.yellow; ctx.lineWidth = 3 * a; ctx.stroke();
      ctx.strokeStyle = P.white; ctx.lineWidth = 1.2 * a; ctx.stroke();
      ctx.translate(t.x, t.y); ctx.fillStyle = P.white;
      for (let i = 0; i < 4; i++) { ctx.rotate(Math.PI / 4); ctx.fillRect(-8 * a, -1.5 * a, 16 * a, 3 * a); }
      ctx.restore();
    }
    for (const r of F.rings) {
      const a = 1 - r.t / r.t0, rad = lerp(r.r0, r.r1, a);
      ctx.save(); ctx.globalAlpha = 1 - a;
      ctx.lineWidth = 6 * (1 - a * .4); ctx.strokeStyle = P.ink; ctx.beginPath(); ctx.arc(r.x, r.y, rad, 0, Math.PI * 2); ctx.stroke();
      ctx.lineWidth = 3 * (1 - a * .3); ctx.strokeStyle = r.col; ctx.stroke();
      ctx.restore();
    }
    for (const p of F.parts) {
      if (p.spark) {
        ctx.save(); ctx.translate(p.x, p.y); ctx.rotate(Math.atan2(p.vy, p.vx));
        ctx.fillStyle = P.ink; ctx.fillRect(-4, -1.5, 8, 3); ctx.fillStyle = p.col; ctx.fillRect(-3.5, -1, 7, 2);
        ctx.restore();
      } else { ctx.fillStyle = P.ink; ctx.fillRect(p.x - 1, p.y - 1, p.s + 2, p.s + 2); ctx.fillStyle = p.col; ctx.fillRect(p.x, p.y, p.s, p.s); }
    }
    for (const f of F.floaters) {
      const prog = 1 - f.t / (f.t0 || .9);
      const sc = prog < .16 ? lerp(.5, 1.32, prog / .16) : prog < .30 ? lerp(1.32, 1, (prog - .16) / .14) : 1;
      ctx.save(); ctx.globalAlpha = Math.min(1, f.t * 3.2); ctx.translate(f.x, f.y); ctx.scale(sc, sc);
      text(f.txt, 0, 0, f.size, f.col, 'center'); ctx.restore();
    }
  }
  function drawIceCrystal(x: number, y: number, rad: number, rot: number, alpha: number) {
    ctx.save(); ctx.translate(x, y); ctx.rotate(rot); ctx.globalAlpha = alpha; ctx.strokeStyle = P.white; ctx.lineWidth = 1.6;
    ctx.beginPath();
    for (let i = 0; i < 6; i++) {
      const a = i * Math.PI / 3, ca = Math.cos(a), sa = Math.sin(a);
      ctx.moveTo(0, 0); ctx.lineTo(ca * rad, sa * rad);
      const b1 = rad * .58, bLen = rad * .32;
      ctx.moveTo(ca * b1, sa * b1); ctx.lineTo(ca * b1 + Math.cos(a + .55) * bLen, sa * b1 + Math.sin(a + .55) * bLen);
      ctx.moveTo(ca * b1, sa * b1); ctx.lineTo(ca * b1 + Math.cos(a - .55) * bLen, sa * b1 + Math.sin(a - .55) * bLen);
    }
    ctx.stroke(); ctx.restore();
  }
  function drawFreezeOverlay() {
    const a = clamp(S.freezeIn / 1.2, 0, 1);
    ctx.fillStyle = `rgba(130,215,255,${.14 * a})`; ctx.fillRect(0, 0, W, H);
    ctx.strokeStyle = `rgba(190,240,255,${.55 * a})`; ctx.lineWidth = 8; ctx.strokeRect(4, 4, W - 8, H - 8);
    const crystals = [[28, 28, 18], [W - 28, 28, 18], [28, H - 28, 18], [W - 28, H - 28, 18], [L.arena.x + 8, L.arena.y + 12, 12], [L.arena.x + L.arena.w - 8, L.arena.y + 12, 12]];
    crystals.forEach(([cx, cy, cr], i) => drawIceCrystal(cx, cy, cr * (.85 + .15 * Math.sin(S.time * 3 + i)), S.time * .3 + i, .78 * a));
  }

  // ---- HUD ----
  function hudPanel(x: number, y: number, w: number, h: number) {
    ctx.save();
    rr(x, y + 2, w, h, 6); ctx.fillStyle = 'rgba(10,10,16,0.38)'; ctx.fill();
    rr(x, y, w, h, 6); ctx.fillStyle = 'rgba(18,22,32,0.58)'; ctx.fill();
    ctx.lineWidth = 1.2; ctx.strokeStyle = 'rgba(243,227,184,0.22)'; ctx.stroke();
    ctx.restore();
  }
  function bar(x: number, y: number, w: number, h: number, frac: number, col: string, trailFrac?: number, spark?: boolean) {
    const f = clamp(frac, 0, 1), fw = Math.round(w * f);
    ctx.fillStyle = P.ink; ctx.fillRect(x - 1.5, y - 1.5, w + 3, h + 3);
    ctx.fillStyle = '#252019'; ctx.fillRect(x, y, w, h);
    if (trailFrac !== undefined && trailFrac > f) {
      const tw = Math.round(w * clamp(trailFrac, 0, 1));
      ctx.fillStyle = '#ff6b57'; ctx.fillRect(x + fw, y, Math.max(0, tw - fw), h);
      ctx.fillStyle = 'rgba(255,255,255,0.6)'; ctx.fillRect(x + fw, y, Math.max(0, tw - fw), Math.max(1, h * .4));
    }
    if (fw > 0) {
      ctx.fillStyle = col; ctx.fillRect(x, y, fw, h);
      ctx.fillStyle = 'rgba(255,255,255,0.34)'; ctx.fillRect(x, y, fw, Math.max(1, Math.floor(h * .35)));
    }
    if (w >= 80 && h >= 6) { ctx.fillStyle = 'rgba(20,20,20,0.38)'; for (let i = 1; i < 4; i++) ctx.fillRect(Math.round(x + (w * i) / 4), y, 1, h); }
    if (spark && f > .02) {
      const sx = x + fw, sy = y + h * .5, pulse = 2.2 + Math.sin(S.time * 24) * 1.1;
      ctx.fillStyle = P.white; ctx.fillRect(sx - 1.5, sy - pulse, 3, pulse * 2);
      ctx.fillStyle = P.yellow; ctx.fillRect(sx - 2.5, sy - 1.5, 5, 3);
    }
  }
  const hpCol = () => S.hp > 50 ? P.green : S.hp > 25 ? P.yellow : P.red;
  function abilityButton(x: number, y: number, label: string, key: string, ready: boolean, active: boolean, frac?: number) {
    ctx.save(); ctx.translate(x, y);
    ctx.beginPath(); for (let i = 0; i < 6; i++) { const a = i * Math.PI / 3 + Math.PI / 6; ctx.lineTo(Math.cos(a) * 16, Math.sin(a) * 16 + 2.5); } ctx.closePath();
    ctx.fillStyle = P.ink; ctx.fill();
    ctx.beginPath(); for (let i = 0; i < 6; i++) { const a = i * Math.PI / 3 + Math.PI / 6; ctx.lineTo(Math.cos(a) * 15, Math.sin(a) * 15); } ctx.closePath();
    fillOut(active ? (Math.floor(S.time * 8) % 2 ? P.orange : P.yellow) : ready ? '#4d5260' : '#2c303a', 2.2);
    if (frac !== undefined && frac < 1) {
      ctx.beginPath(); ctx.moveTo(0, 0); ctx.arc(0, 0, 11, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * frac); ctx.closePath();
      ctx.fillStyle = P.ice; ctx.globalAlpha = .55; ctx.fill(); ctx.globalAlpha = 1;
    } else if (ready) { ctx.beginPath(); ctx.arc(0, 0, 11, 0, Math.PI * 2); ctx.strokeStyle = active ? P.white : P.ice; ctx.lineWidth = 1.2; ctx.stroke(); }
    text(label, 0, 0, 9, ready ? P.cream : P.grey, 'center');
    ctx.restore();
    text(key, x, y + 22, 8, P.cream, 'center');
  }
  const beat = () => S.playMode === 'beat';
  function drawBeatPulse() {
    const a = L.arena, down = (S.beatCount - 1) % 4 === 0;
    ctx.globalAlpha = F.beatFlash * (down ? .9 : .5); ctx.lineWidth = down ? 5 : 3; ctx.strokeStyle = down ? P.pink : P.yellow;
    rr(a.x - 8, a.y - 4, a.w + 16, a.h + 12, 8); ctx.stroke(); ctx.globalAlpha = 1;
  }
  function beatDots(x: number, y: number) {
    if (!beat()) return;
    const pos = (S.beatCount - 1 + 4) % 4;
    for (let i = 0; i < 4; i++) { ell(x + (i - 1.5) * 14, y, 4.5, 4.5); fillOut(i === pos && S.mode === 'play' ? (i === 0 ? P.pink : P.yellow) : '#3a3226', 1.5); }
    text(`${Math.round(beatsPerMinute(S))} BPM`, x + 36, y, 8, P.cream2, 'left');
  }

  function drawHUD() {
    const rk = rankOf(S.combo), rc = { S: P.pink, A: P.orange, B: P.yellow, C: P.green, D: P.grey }[rk]!;
    const p = 1 - F.rankPop, pop = F.rankPop > 0 ? 1 + Math.sin(p * Math.PI * 2.2) * F.rankPop * .72 : 1;
    const mult = multiplier(S), pad = L.kind === 'pad', VW = L.VW || W;
    const leftCx = pad ? Math.max(72, L.arena.x - 86) : 40;
    const rx = pad ? leftCx : 40, ry = pad ? 200 : 318, sx = pad ? leftCx : 118, sy = pad ? 262 : 318;
    if (pad) {
      hudPanel(leftCx - 58, 44, 116, 86);
      hudPanel(leftCx - 54, 156, 108, 156);
    } else {
      hudPanel(12, 294, 198, 56);
      hudPanel(VW / 2 - 112, 298, 224, 52);
    }
    ctx.save(); ctx.translate(rx, ry);
    if (F.rankPop > .15) {
      ctx.save(); const g = 1 + (1 - F.rankPop) * 1.4; ctx.scale(g, g); ctx.rotate(-.12); ctx.globalAlpha = F.rankPop * .55;
      text(rk, 0, 0, pad ? 64 : 46, P.white, 'center', false); ctx.restore();
    }
    ctx.scale(pop, pop); ctx.rotate(-.12 + F.rankPop * .18); text(rk, 0, 0, pad ? 64 : 46, rc, 'center'); ctx.restore();
    if (pad) {
      text(`COMBO ×${S.combo}`, rx, ry + 40, 10, P.cream, 'center');
      bar(rx - 42, ry + 50, 84, 5, S.comboIn / 3, rc, undefined, true);
      text(`MULT ×${mult}`, rx, ry + 64, 9, P.yellow, 'center');
      text(String(S.score).padStart(6, '0'), sx, sy + 34, 18, P.cream, 'center');
      text(`WAVE ${S.wave}`, leftCx, 58, 14, P.cream, 'center');
      bar(leftCx - 50, 72, 100, 6, S.waveTotal ? S.waveDone / S.waveTotal : 0, P.teal);
      text('HP', leftCx - 50, 94, 10, P.cream, 'left');
      bar(leftCx - 30, 90, 80, 8, S.hp / 100, hpCol(), F.hpTrail / 100);
      text(`BEST ×${S.best}`, leftCx, 115, 9, P.cream2, 'center');
      beatDots(leftCx - 16, 140);
      for (let r = 8; r >= 1; r--) {
        const R = L.side!(r), cx = R.x + R.w / 2, cy = R.y + R.h / 2;
        ell(cx, cy + 1.5, 13.5, 13.5); ctx.fillStyle = 'rgba(12,12,18,0.75)'; ctx.fill();
        if (r === 8) {
          const lit = S.armed || S.bombs > 0;
          ell(cx, cy, 12, 12); fillOut(S.armed ? (Math.floor(S.time * 8) % 2 ? P.orange : P.yellow) : S.bombs > 0 ? P.orange2 : '#343640', 2);
          if (lit) { ell(cx, cy - 2.5, 8, 5.5); ctx.fillStyle = 'rgba(255,255,255,0.22)'; ctx.fill(); }
          text(`B${S.bombs}`, cx, cy, 8, P.cream, 'center');
        } else if (r === 7) {
          const lit = S.charge >= 100 || S.freezeIn > 0;
          ell(cx, cy, 12, 12); fillOut(lit ? P.ice : '#263542', 2);
          if (!lit && S.charge > 0) {
            ctx.beginPath(); ctx.moveTo(cx, cy); ctx.arc(cx, cy, 10, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * (S.charge / 100)); ctx.closePath();
            ctx.fillStyle = 'rgba(168,230,255,0.45)'; ctx.fill();
          }
          if (lit) { ell(cx, cy - 2.5, 8, 5.5); ctx.fillStyle = 'rgba(255,255,255,0.35)'; ctx.fill(); }
          text(S.freezeIn > 0 ? '❄' : S.charge >= 100 ? 'ICE' : String(Math.floor(S.charge)), cx, cy, 8, S.charge >= 100 ? P.ink : P.cream, 'center', S.charge < 100);
        } else {
          const lit = S.hp / 100 * 6 >= r - .5;
          ell(cx, cy, 12, 12); fillOut(lit ? hpCol() : '#25262e', 2);
          if (lit) { ell(cx, cy - 3, 7.5, 4.5); ctx.fillStyle = 'rgba(255,255,255,0.26)'; ctx.fill(); }
        }
      }
      text('BOMB', L.side!(8).x + 40, L.side!(8).y + 18, 8, P.cream, 'left');
      text('FREEZE', L.side!(7).x + 40, L.side!(7).y + 18, 8, P.cream, 'left');
      text('BASE HP', L.side!(4).x + 40, L.side!(4).y + 18, 8, P.cream, 'left');
      const rightPanelX = Math.min(VW - 100, L.arena.x + L.arena.w + 64);
      hudPanel(rightPanelX, 286, 92, 42);
      text(`KILLS ${S.kills}`, rightPanelX + 46, 299, 9, P.cream2, 'center');
      text(`SAVED ${S.saved}`, rightPanelX + 46, 315, 9, P.teal, 'center');
    } else {
      text(`×${S.combo}`, rx + 24, ry - 10, 10, P.cream, 'left');
      bar(rx + 24, ry, 54, 5, S.comboIn / 3, rc, undefined, true);
      text(String(S.score).padStart(6, '0'), sx + 40, sy + 20, 16, P.cream, 'center');
      text(`MULT ×${mult}   BEST ×${S.best}`, sx + 40, sy - 14, 8, P.yellow, 'center');
      text(`WAVE ${S.wave}`, VW / 2, 310, 11, P.cream, 'center');
      beatDots(VW / 2 - 150, 310);
      bar(VW / 2 - 100, 321, 200, 8, S.hp / 100, hpCol(), F.hpTrail / 100);
      text(`${S.hp} / 100`, VW / 2, 340, 9, P.cream, 'center');
      abilityButton(VW - 120, 318, `B${S.bombs}`, 'SHIFT', S.bombs > 0, S.armed);
      abilityButton(VW - 50, 318, 'ICE', 'SPACE', S.charge >= 100, S.freezeIn > 0, S.charge / 100);
    }
  }

  function drawAdditiveGlow() {
    if (cfg.quality === 'low') return;
    const VW = L.VW || W;
    ctx.save(); ctx.globalCompositeOperation = 'lighter';
    for (const e of S.enemies) {
      if (e.state === 'gate') continue;
      cellsOf(e).forEach(([c, r], i) => {
        const R = L.rect(c, r);
        if (sizeOf(e) === 2 && i !== (e.weak ?? 0)) return;
        ctx.globalAlpha = sizeOf(e) === 2 ? .24 : .14;
        ctx.fillStyle = S.freezeIn > 0 ? P.ice : sizeOf(e) === 2 ? P.pink : LOOK[e.type].c1;
        ell(R.x + R.w * .5, R.y + R.h * .5, R.w * .56, R.h * .52); ctx.fill();
      });
    }
    if (F.muzzleT > 0) {
      ctx.globalAlpha = .28; ctx.fillStyle = P.orange;
      ell(L.turret.x + Math.cos(F.turretAng) * 18, L.turret.y + Math.sin(F.turretAng) * 18, 18, 18); ctx.fill();
    }
    for (const t of F.tracers) {
      const a = clamp(t.t / (t.t0 || .09), 0, 1);
      ctx.globalAlpha = .26 * a; ctx.strokeStyle = S.armed ? P.orange : P.yellow; ctx.lineWidth = 9 * a;
      ctx.beginPath(); ctx.moveTo(L.turret.x, L.turret.y); ctx.lineTo(t.x, t.y); ctx.stroke();
      ctx.fillStyle = P.yellow; ell(t.x, t.y, 14 * a, 14 * a); ctx.fill();
    }
    for (const f of S.cellFx) {
      if (f.kind !== 'bomb' && f.kind !== 'hit') continue;
      const R = L.rect(f.column, f.row), a = clamp(f.t / f.t0, 0, 1);
      ctx.globalAlpha = (f.kind === 'bomb' ? .25 : .16) * a; ctx.fillStyle = f.kind === 'bomb' ? P.orange : P.ice;
      ell(R.x + R.w * .5, R.y + R.h * .5, R.w * .6, R.h * .6); ctx.fill();
    }
    bloomCtx.clearRect(0, 0, bloomCv.width, bloomCv.height);
    bloomCtx.drawImage(canvas, 0, 0, bloomCv.width, bloomCv.height);
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.globalCompositeOperation = 'screen'; ctx.globalAlpha = .16; ctx.imageSmoothingEnabled = true;
    ctx.drawImage(bloomCv, 0, 0, VW, H);
    ctx.imageSmoothingEnabled = false;
    ctx.restore();
  }

  function drawWaveBanner() {
    const b = F.banner;
    if (!b) return;
    const VW = L.VW || W;
    const t0 = b.t0 || 2.4, elapsed = t0 - b.t;
    const enter = clamp(elapsed / .22, 0, 1), exit = clamp(b.t / .24, 0, 1), a = Math.min(enter, exit);
    const slideX = enter < 1 ? (1 - enter) * (1 - enter) * -VW * .65 : exit < 1 ? (1 - exit) * (1 - exit) * VW * .65 : 0;
    const isBoss = S.wave % 3 === 0 && S.mode === 'play';
    ctx.save(); ctx.globalAlpha = a; ctx.translate(slideX, 0);
    ctx.fillStyle = 'rgba(14,16,24,0.92)'; ctx.fillRect(-VW, 136, VW * 3, 62);
    ctx.fillStyle = isBoss ? P.red : P.teal; ctx.fillRect(-VW, 136, VW * 3, 5); ctx.fillRect(-VW, 193, VW * 3, 5);
    ctx.fillStyle = P.yellow;
    const stripeOff = (S.time * 40) % 24;
    for (let x = -40 + stripeOff; x < VW + 40; x += 24) { ctx.fillRect(x, 136, 10, 5); ctx.fillRect(x, 193, 10, 5); }
    text(b.text, VW / 2, 160, 24, isBoss ? P.red : P.yellow, 'center');
    if (b.sub) text(b.sub, VW / 2, 182, 9, P.cream, 'center');
    ctx.restore();
  }

  function drawMiniBadge(type: EnemyType, cx: number, cy: number) {
    const look = LOOK[type];
    ctx.save(); ctx.translate(cx, cy);
    rr(-9, -8, 18, 16, 4); fillOut(look.c2, 1.5);
    rr(-7.5, -6.5, 15, 12, 3); ctx.fillStyle = look.c1; ctx.fill();
    ctx.fillStyle = P.ink;
    if (type === 'grunt') { ell(-3, -1, 2, 2); ctx.fill(); ell(3, -1, 2, 2); ctx.fill(); }
    else if (type === 'runner') { ctx.beginPath(); ctx.moveTo(-5, -4); ctx.lineTo(0, 4); ctx.lineTo(5, -4); ctx.closePath(); ctx.fill(); }
    else if (type === 'brute') { ctx.fillRect(-5, -3, 10, 6); ctx.fillStyle = P.cream; ctx.fillRect(-6, -6, 3, 3); ctx.fillRect(3, -6, 3, 3); }
    else if (type === 'splitter') { ctx.beginPath(); ctx.moveTo(0, -6); ctx.lineTo(-2, 0); ctx.lineTo(2, 0); ctx.lineTo(0, 6); ctx.strokeStyle = P.white; ctx.lineWidth = 1.5; ctx.stroke(); }
    else if (type === 'survivor') { ell(0, -1, 3.5, 3.5); fillOut(P.orange, 1); }
    else { ell(0, 0, 4, 4); fillOut(P.pink, 1.2); }
    ctx.restore();
  }
  function drawTitle() {
    const VW = L.VW || W;
    ctx.fillStyle = 'rgba(10,12,24,.76)'; ctx.fillRect(0, 0, VW, H);
    hudPanel(VW / 2 - 220, 40, 440, 286);
    text('HOLD THE BEACH', VW / 2, 70, 36, P.yellow, 'center');
    text('a horde defense for the Launchpad (or your keyboard)', VW / 2, 102, 10, P.cream, 'center');
    const rows: [EnemyType, string][] = [['grunt', '1 hit · gill-fin waddler'], ['runner', '1 hit · fast darting zig-zagger'],
      ['brute', '3 hits · horned tank, pushed back'], ['splitter', '2 hits · splits in two along seam'],
      ['survivor', 'DON\'T SHOOT · life-ring castaway'], ['boss', '2×2 CRAB KING · hit flashing weak core']];
    rows.forEach(([t, sub], i) => {
      const y = 134 + i * 20;
      drawMiniBadge(t, VW / 2 - 158, y);
      text(ENEMY[t].name, VW / 2 - 138, y, 10, LOOK[t].c1, 'left');
      text(sub, VW / 2 - 46, y, 9, P.cream, 'left');
    });
    text(L.kind === 'pad' ? 'PADS = SHOOT · SIDE ▶ TOP = BOMB · NEXT = FREEZE' : 'KEYS = SHOOT · SHIFT+KEY = BOMB · SPACE = FREEZE', VW / 2, 266, 10, P.teal, 'center');
    text(beat() ? 'BEAT MODE: THE HORDE MOVES ON THE BEAT · HIT ON THE BEAT FOR ×2' : 'FREE MODE · SWITCH TO BEAT MODE IN THE BAR', VW / 2, 282, 9, beat() ? P.pink : P.cream2, 'center');
    text(Math.floor(S.time * 2) % 2 ? (L.kind === 'pad' ? 'PRESS ANY PAD TO START' : 'PRESS ANY KEY TO START') : '', VW / 2, 306, 13, P.yellow, 'center');
  }
  function drawResults() {
    const VW = L.VW || W;
    ctx.fillStyle = 'rgba(10,12,24,.78)'; ctx.fillRect(0, 0, VW, H);
    hudPanel(VW / 2 - 190, 42, 380, 256);
    text('THE BEACH HAS FALLEN', VW / 2, 72, 26, P.red, 'center');
    const acc = S.shots ? Math.round(100 * S.hits / S.shots) : 0;
    const lines = [`SCORE ${S.score}`, `WAVE REACHED ${S.wave}`, `KILLS ${S.kills}   SURVIVORS SAVED ${S.saved}`, `BEST COMBO ×${S.best}`,
      `ACCURACY ${acc}%  (${S.hits}/${S.shots})`, ...(beat() ? [`ON-BEAT HITS ${S.perfect}`] : [])];
    lines.forEach((line, i) => text(line, VW / 2, 114 + i * 22, 12, P.cream, 'center'));
    if (S.modeT > 1.2) text(Math.floor(S.time * 2) % 2 ? 'PRESS ANY PAD OR KEY TO PLAY AGAIN' : '', VW / 2, 272, 12, P.yellow, 'center');
  }

  function postProcess() {
    if (cfg.quality === 'low' || !cfg.crush) return;
    const VW = L.VW || W;
    const lut = crushLut(cfg.crush);
    const img = ctx.getImageData(0, 0, VW, H), d = img.data;
    for (let y = 0; y < H; y++) {
      const by = (y & 3) * 4;
      let i = y * VW * 4;
      for (let x = 0; x < VW; x++, i += 4) {
        const shift = (by + (x & 3)) << 8;
        d[i] = lut[shift | d[i]]; d[i + 1] = lut[shift | d[i + 1]]; d[i + 2] = lut[shift | d[i + 2]];
      }
    }
    ctx.putImageData(img, 0, 0);
  }

  function draw(state: HordeState, fx: Fx, layout: ArenaLayout, settings: HordeSettings) {
    S = state; F = fx; L = layout; cfg = settings;
    const targetW = L.VW || W;
    if (sandCv.width !== targetW) {
      curVW = targetW;
      sandCv.width = curVW;
      boardCv.width = curVW;
      bloomCv.width = Math.round(curVW / 4);
      cacheKey = '';
    }
    const VW = curVW;
    const index = themeIndex(S.wave);
    theme = THEMES[index];
    const key = `${L.kind}|${index}|${cfg.aids}|${VW}`;
    if (key !== cacheKey) { cacheKey = key; buildSandCache(); }
    ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.fillStyle = '#000'; ctx.fillRect(0, 0, VW, H);
    ctx.imageSmoothingEnabled = false;
    const reduce = cfg.motion === 'reduce', shake = reduce ? 0 : F.shake;
    if (shake > 0) {
      const dx = Math.round((Math.random() * 2 - 1) * shake * .55), dy = Math.round((Math.random() * 2 - 1) * shake * .55);
      ctx.translate(VW / 2 + dx, H / 2 + dy);
      if (S.hurtT > 0) ctx.rotate((Math.random() * 2 - 1) * .015 * (shake / 10));
      ctx.translate(-VW / 2, -H / 2);
    }
    drawSea(); drawBeach();
    if (beat() && S.mode === 'play') drawBeatPulse();
    drawDecals(); drawCellFx(); drawTelegraphs(); drawBase();
    for (const e of [...S.enemies].sort((a, b) => enemyPos(a).y - enemyPos(b).y)) drawEnemy(e);
    drawFx();
    drawAdditiveGlow();
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    drawHUD();
    if (S.freezeIn > 0) drawFreezeOverlay();
    if (S.hurtT > 0 && !reduce) { ctx.globalAlpha = S.hurtT; ctx.lineWidth = 18; ctx.strokeStyle = P.red; ctx.strokeRect(0, 0, VW, H); ctx.globalAlpha = 1; }
    if (S.armed) text('BOMB ARMED: HIT A CELL', VW / 2, L.kind === 'pad' ? 50 : 74, 11, P.orange, 'center');
    drawWaveBanner();
    if (S.paused) { ctx.fillStyle = 'rgba(0,0,0,.6)'; ctx.fillRect(0, 0, VW, H); text('PAUSED', VW / 2, H / 2, 24, P.yellow, 'center'); text('P TO RESUME', VW / 2, H / 2 + 24, 10, P.cream, 'center'); }
    if (S.mode === 'title') drawTitle(); else if (S.mode === 'over') drawResults();
    postProcess();
  }

  return { draw, invalidate() { cacheKey = ''; } };
}
export type HordeRenderer = ReturnType<typeof createRenderer>;
