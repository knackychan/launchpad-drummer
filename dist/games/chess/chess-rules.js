(function installRules(root) {
'use strict';
const FILES = 'abcdefgh';
const opp = c => c === 'w' ? 'b' : 'w';
const fileOf = i => i & 7, rankOf = i => i >> 3;
const sqName = i => FILES[fileOf(i)] + (rankOf(i)+1);
let NEXT_ID = 1;
function initialState() {
  const b = new Array(64).fill(null), back = 'rnbqkbnr';
  for (let f = 0; f < 8; f++) {
    b[f] = { t:back[f], c:'w', id:NEXT_ID++ }; b[8+f] = { t:'p', c:'w', id:NEXT_ID++ };
    b[48+f] = { t:'p', c:'b', id:NEXT_ID++ }; b[56+f] = { t:back[f], c:'b', id:NEXT_ID++ };
  }
  return { b, turn:'w', castle:{ wK:true, wQ:true, bK:true, bQ:true }, ep:-1, half:0, full:1 };
}
const N_OFF = [[1,2],[2,1],[2,-1],[1,-2],[-1,-2],[-2,-1],[-2,1],[-1,2]];
const K_OFF = [[1,0],[1,1],[0,1],[-1,1],[-1,0],[-1,-1],[0,-1],[1,-1]];
const DIAG = [[1,1],[1,-1],[-1,1],[-1,-1]], ORTH = [[1,0],[-1,0],[0,1],[0,-1]];
function at(s, f, r) { return (f<0||f>7||r<0||r>7) ? undefined : s.b[r*8+f]; }
function attackers(s, sq, by, firstOnly=false) {
  const f = fileOf(sq), r = rankOf(sq), out = [];
  const hit = (x,y) => { out.push(y*8+x); return firstOnly; };
  const pr = by === 'w' ? -1 : 1;   // an attacking white pawn sits one rank below
  for (const df of [-1,1]) { const p = at(s,f+df,r+pr); if (p && p.c===by && p.t==='p' && hit(f+df,r+pr)) return out; }
  for (const [df,dr] of N_OFF) { const p = at(s,f+df,r+dr); if (p && p.c===by && p.t==='n' && hit(f+df,r+dr)) return out; }
  for (const [df,dr] of K_OFF) { const p = at(s,f+df,r+dr); if (p && p.c===by && p.t==='k' && hit(f+df,r+dr)) return out; }
  for (const [dirs, types] of [[DIAG,'bq'],[ORTH,'rq']]) for (const [df,dr] of dirs) {
    let x = f+df, y = r+dr;
    for (;;) { const p = at(s,x,y); if (p === undefined) break; if (p) { if (p.c===by && types.includes(p.t) && hit(x,y)) return out; break; } x += df; y += dr; }
  }
  return out;
}
const attacked = (s, sq, by) => attackers(s, sq, by, true).length > 0;
function kingSq(s, c) { for (let i = 0; i < 64; i++) { const p = s.b[i]; if (p && p.t==='k' && p.c===c) return i; } return -1; }
const inCheck = (s, c) => attacked(s, kingSq(s,c), opp(c));
function pseudo(s, from) {
  const p = s.b[from], out = [], f = fileOf(from), r = rankOf(from), c = p.c, o = opp(c);
  const add = (to, extra) => out.push(Object.assign({ from, to, piece:p, cap:s.b[to]||null, capSq:to }, extra));
  if (p.t === 'p') {
    const d = c==='w' ? 1 : -1, start = c==='w' ? 1 : 6, last = c==='w' ? 7 : 0;
    const push = (to, extra) => { if (rankOf(to) === last) for (const pr of 'qrbn') add(to, Object.assign({ promo:pr }, extra)); else add(to, extra); };
    if (at(s,f,r+d) === null) { push((r+d)*8+f); if (r === start && at(s,f,r+2*d) === null) add((r+2*d)*8+f, { double:true }); }
    for (const df of [-1,1]) {
      const q = at(s,f+df,r+d); if (q === undefined) continue; const to = (r+d)*8+f+df;
      if (q && q.c === o) push(to);
      else if (q === null && to === s.ep) add(to, { ep:true, cap:s.b[r*8+f+df], capSq:r*8+f+df });
    }
  } else if (p.t === 'n' || p.t === 'k') {
    for (const [df,dr] of (p.t==='n' ? N_OFF : K_OFF)) { const q = at(s,f+df,r+dr); if (q === undefined || (q && q.c === c)) continue; add((r+dr)*8+f+df); }
    if (p.t === 'k') {
      const rk = c==='w' ? 0 : 7, home = rk*8+4;
      if (from === home && !attacked(s, home, o)) {
        const rookOk = sq => { const q = s.b[sq]; return q && q.t==='r' && q.c===c; };
        if (s.castle[c+'K'] && !s.b[home+1] && !s.b[home+2] && rookOk(home+3) && !attacked(s,home+1,o) && !attacked(s,home+2,o)) add(home+2, { castle:'K', rookFrom:home+3, rookTo:home+1 });
        if (s.castle[c+'Q'] && !s.b[home-1] && !s.b[home-2] && !s.b[home-3] && rookOk(home-4) && !attacked(s,home-1,o) && !attacked(s,home-2,o)) add(home-2, { castle:'Q', rookFrom:home-4, rookTo:home-1 });
      }
    }
  } else {
    const dirs = p.t==='b' ? DIAG : p.t==='r' ? ORTH : DIAG.concat(ORTH);
    for (const [df,dr] of dirs) { let x = f+df, y = r+dr; for (;;) { const q = at(s,x,y); if (q === undefined || (q && q.c === c)) break; add(y*8+x); if (q) break; x += df; y += dr; } }
  }
  return out;
}
function applyMove(s, m) {
  const b = s.b.slice(), p = m.piece, c = p.c;
  b[m.from] = null; if (m.ep) b[m.capSq] = null;
  b[m.to] = m.promo ? { t:m.promo, c, id:p.id } : p;
  if (m.castle) { b[m.rookTo] = b[m.rookFrom]; b[m.rookFrom] = null; }
  const castle = Object.assign({}, s.castle);
  if (p.t === 'k') { castle[c+'K'] = false; castle[c+'Q'] = false; }
  for (const sq of [m.from, m.to]) { if (sq===0) castle.wQ=false; if (sq===7) castle.wK=false; if (sq===56) castle.bQ=false; if (sq===63) castle.bK=false; }
  return { b, turn:opp(c), castle, ep: m.double ? (m.from+m.to)/2 : -1, half: (p.t==='p'||m.cap) ? 0 : s.half+1, full: s.full + (c==='b'?1:0) };
}
function legalMoves(s, from = -1) {
  const out = [];
  for (let i = 0; i < 64; i++) {
    if (from >= 0 && i !== from) continue;
    const p = s.b[i]; if (!p || p.c !== s.turn) continue;
    for (const m of pseudo(s, i)) if (!inCheck(applyMove(s, m), s.turn)) out.push(m);
  }
  return out;
}
function posKey(s) { let k = ''; for (const p of s.b) k += p ? (p.c==='w' ? p.t.toUpperCase() : p.t) : '.'; return k + s.turn + (s.castle.wK?'K':'') + (s.castle.wQ?'Q':'') + (s.castle.bK?'k':'') + (s.castle.bQ?'q':'') + (s.ep >= 0 && legalMoves(s).some(m => m.ep) ? s.ep : -1); }
function san(s, m, all) {
  let t;
  if (m.castle) t = m.castle === 'K' ? 'O-O' : 'O-O-O';
  else {
    const P = m.piece.t; let dis = '';
    if (P === 'p') { if (m.cap) dis = FILES[fileOf(m.from)]; }
    else {
      const others = all.filter(o => o.to===m.to && o.piece.t===P && o.from!==m.from);
      if (others.length) { const sf = others.some(o => fileOf(o.from)===fileOf(m.from)), sr = others.some(o => rankOf(o.from)===rankOf(m.from)); dis = !sf ? FILES[fileOf(m.from)] : !sr ? String(rankOf(m.from)+1) : sqName(m.from); }
    }
    t = (P==='p' ? '' : P.toUpperCase()) + dis + (m.cap ? 'x' : '') + sqName(m.to) + (m.promo ? '=' + m.promo.toUpperCase() : '');
  }
  const ns = applyMove(s, m);
  if (inCheck(ns, ns.turn)) t += legalMoves(ns).length ? '+' : '#';
  return t;
}
function insufficient(s) {
  const minors = []; for (let i = 0; i < 64; i++) { const p = s.b[i]; if (!p || p.t === 'k') continue; if (p.t==='p'||p.t==='r'||p.t==='q') return false; minors.push({ t:p.t, c:p.c, sq:(fileOf(i)+rankOf(i))&1 }); }
  if (minors.length <= 1) return true;
  return minors.every(m => m.t === 'b') && minors.every(m => m.sq === minors[0].sq);
}
function resetIds() { NEXT_ID = 1; }
function drawReason(s, keys) {
  if (s.half >= 100) return '50-MOVE RULE';
  if (keys.filter(k => k === posKey(s)).length >= 3) return 'THREEFOLD REPETITION';
  if (insufficient(s)) return 'NOT ENOUGH MATERIAL';
  return null;
}
const rules = { initialState, legalMoves, applyMove, inCheck, posKey, san, resetIds, drawReason, kingSq, attackers };
rules.workerSource = '(' + installRules.toString() + ')(self);';
root.GameRules = rules;
if (typeof module !== 'undefined' && module.exports) module.exports = rules;
})(typeof globalThis !== 'undefined' ? globalThis : this);
