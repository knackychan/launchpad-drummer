(function installRules(root) {
'use strict';
const NF=9, NR=10, NSQ=90, FILES='abcdefghi';
const LETTER = { k:'K', a:'A', e:'E', n:'H', r:'R', c:'C', p:'P' };
const opp = c => c === 'r' ? 'b' : 'r';
const fileOf = i => i % NF, rankOf = i => Math.floor(i / NF);
const sqName = i => FILES[fileOf(i)] + rankOf(i);
let NEXT_ID = 1;
function initialState() {
  const b = new Array(NSQ).fill(null), back = 'rneakaenr';
  const put = (f, r, t, c) => b[r*NF+f] = { t, c, id:NEXT_ID++ };
  for (let f = 0; f < 9; f++) { put(f, 0, back[f], 'r'); put(f, 9, back[f], 'b'); }
  put(1, 2, 'c', 'r'); put(7, 2, 'c', 'r'); put(1, 7, 'c', 'b'); put(7, 7, 'c', 'b');
  for (const f of [0,2,4,6,8]) { put(f, 3, 'p', 'r'); put(f, 6, 'p', 'b'); }
  return { b, turn:'r', half:0, full:1 };
}
function at(s, f, r) { return (f<0||f>=NF||r<0||r>=NR) ? undefined : s.b[r*NF+f]; }
const inPalace = (c, f, r) => f >= 3 && f <= 5 && (c === 'r' ? r <= 2 : r >= 7);
const ownHalf = (c, r) => c === 'r' ? r <= 4 : r >= 5;
const ORTH = [[1,0],[-1,0],[0,1],[0,-1]], DIAG = [[1,1],[1,-1],[-1,1],[-1,-1]];
function pseudo(s, from) {
  const p = s.b[from], out = [], f = fileOf(from), r = rankOf(from), c = p.c;
  const add = (x, y) => { const q = at(s,x,y); if (q === undefined || (q && q.c === c)) return; out.push({ from, to:y*NF+x, piece:p, cap:q||null, capSq:y*NF+x }); };
  switch (p.t) {
    case 'k': for (const [dx,dy] of ORTH) if (inPalace(c, f+dx, r+dy)) add(f+dx, r+dy); break;
    case 'a': for (const [dx,dy] of DIAG) if (inPalace(c, f+dx, r+dy)) add(f+dx, r+dy); break;
    case 'e': for (const [dx,dy] of DIAG) if (at(s,f+dx,r+dy) === null && ownHalf(c, r+2*dy)) add(f+2*dx, r+2*dy); break;   // blocked at the "elephant eye", never crosses the river
    case 'n': for (const [dx,dy] of ORTH) { if (at(s,f+dx,r+dy) !== null) continue;   // hobbled horse leg
      if (dx) { add(f+2*dx, r+1); add(f+2*dx, r-1); } else { add(f+1, r+2*dy); add(f-1, r+2*dy); } } break;
    case 'r': for (const [dx,dy] of ORTH) { let x = f+dx, y = r+dy; for (;;) { const q = at(s,x,y); if (q === undefined) break; if (q === null) { add(x,y); x += dx; y += dy; continue; } add(x,y); break; } } break;
    case 'c': for (const [dx,dy] of ORTH) { let x = f+dx, y = r+dy, screen = false;
      for (;;) { const q = at(s,x,y); if (q === undefined) break;
        if (!screen) { if (q === null) add(x,y); else screen = true; }
        else if (q) { if (q.c !== c) add(x,y); break; }
        x += dx; y += dy; } } break;
    case 'p': { const d = c === 'r' ? 1 : -1; add(f, r+d); if (!ownHalf(c, r)) { add(f+1, r); add(f-1, r); } break; }
  }
  return out;
}
function generalSq(s, c) { for (let i = 0; i < NSQ; i++) { const p = s.b[i]; if (p && p.t==='k' && p.c===c) return i; } return -1; }
function generalsFace(s) {   // the "flying general" rule: the two generals may never see each other on an open file
  const a = generalSq(s,'r'), b = generalSq(s,'b'); if (a < 0 || b < 0 || fileOf(a) !== fileOf(b)) return false;
  for (let r = rankOf(a)+1; r < rankOf(b); r++) if (s.b[r*NF+fileOf(a)]) return false;
  return true;
}
function attackers(s, sq, by) { const out = []; for (let i = 0; i < NSQ; i++) { const p = s.b[i]; if (p && p.c === by && pseudo(s, i).some(m => m.to === sq)) out.push(i); } return out; }
const inCheck = (s, c) => attackers(s, generalSq(s,c), opp(c)).length > 0;
function applyMove(s, m) {
  const b = s.b.slice(); b[m.from] = null; b[m.to] = m.piece;
  return { b, turn:opp(m.piece.c), half: m.cap ? 0 : s.half+1, full: s.full + (m.piece.c==='b'?1:0) };
}
function legalMoves(s, from = -1) {
  const out = [];
  for (let i = 0; i < NSQ; i++) {
    if (from >= 0 && i !== from) continue;
    const p = s.b[i]; if (!p || p.c !== s.turn) continue;
    for (const m of pseudo(s, i)) { const ns = applyMove(s, m); if (!inCheck(ns, s.turn) && !generalsFace(ns)) out.push(m); }
  }
  return out;
}
function posKey(s) { let k = ''; for (const p of s.b) k += p ? (p.c==='r' ? p.t.toUpperCase() : p.t) : '.'; return k + s.turn; }
function san(s, m) {   // simple coordinate notation: piece letter, from, to (x = capture), + check, # mate
  let t = LETTER[m.piece.t] + sqName(m.from) + (m.cap ? 'x' : '-') + sqName(m.to);
  const ns = applyMove(s, m);
  if (inCheck(ns, ns.turn)) t += legalMoves(ns).length ? '+' : '#';
  return t;
}
function noAttackers(s) { return !s.b.some(p => p && 'rncp'.includes(p.t)); }
function resetIds() { NEXT_ID = 1; }
function drawReason(s, keys) {
  if (s.half >= 120) return '60 MOVES WITHOUT CAPTURE';
  if (keys.filter(k => k === posKey(s)).length >= 3) return 'THREEFOLD REPETITION';
  if (noAttackers(s)) return 'NO ATTACKING PIECES LEFT';
  return null;
}
const rules = { initialState, legalMoves, applyMove, inCheck, posKey, san, resetIds, drawReason, generalSq, attackers, generalsFace };
rules.workerSource = '(' + installRules.toString() + ')(self);';
root.GameRules = rules;
if (typeof module !== 'undefined' && module.exports) module.exports = rules;
})(typeof globalThis !== 'undefined' ? globalThis : this);
