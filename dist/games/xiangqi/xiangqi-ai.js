/**
 * xiangqi-ai.js - High-Performance Multi-Level Chinese Chess (Xiangqi) AI Engine for LaunchBand
 * 
 * Features:
 * - Specialized Xiangqi Piece-Square Tables (Pawn river crossing, central cannons, open chariots, mobile horses)
 * - Simplified threefold draw, including repeated checks (not tournament adjudication)
 * - 64-bit Zobrist move-ordering cache (no history-dependent score reuse)
 * - Quiescence Search with MVV-LVA capture ordering
 * - Killer Move Heuristic (2 killers per ply) & History Heuristic (90x90)
 * - Iterative Deepening Negamax with Alpha-Beta pruning
 * - 5 difficulty settings (unrated)
 * - Usable in Browser (Main Thread & Web Worker) and Node.js
 */
(function installAI(root) {
  'use strict';

  const NF = 9, NR = 10, NSQ = 90;

  // Base piece values
  const PIECE_VAL = {
    k: 10000,
    r: 900,
    c: 450,
    n: 400,
    e: 200,
    a: 200,
    p: 100
  };

  // ───────────────────────────── Piece-Square Tables (Red perspective: Rank 0 = near side, Rank 9 = far side) ─────────────────────────────
  // Red moves North (rank 0 -> 9). For Black, rank is mirrored (9 - r).
  const PST_SOLDIER_RED = [
      0,   0,   0,   0,   0,   0,   0,   0,   0,  // r0
      0,   0,   0,   0,   0,   0,   0,   0,   0,  // r1
      0,   0,   0,  -2,   4,  -2,   0,   0,   0,  // r2
      0,   0,   0,   0,   0,   0,   0,   0,   0,  // r3 (starting rank)
      0,   0,   0,   0,   0,   0,   0,   0,   0,  // r4 (before river)
     70,  90, 110, 120, 120, 120, 110,  90,  70,  // r5 (crossed river!)
     80, 100, 120, 140, 150, 140, 120, 100,  80,  // r6
     90, 110, 130, 160, 170, 160, 130, 110,  90,  // r7 (approaching palace)
    100, 120, 140, 170, 180, 170, 140, 120, 100,  // r8 (deep in palace)
     50,  60,  70,  80,  80,  80,  70,  60,  50   // r9 (bottom rank)
  ];

  const PST_HORSE = [
     -4,  -8,  -2,  -4,  -6,  -4,  -2,  -8,  -4,  // r0
     -4,   2,   6,   4,   2,   4,   6,   2,  -4,  // r1
      2,   8,  12,  14,  10,  14,  12,   8,   2,  // r2
      4,  10,  16,  20,  18,  20,  16,  10,   4,  // r3
      6,  12,  20,  24,  22,  24,  20,  12,   6,  // r4
     10,  16,  24,  28,  26,  28,  24,  16,  10,  // r5
     10,  18,  26,  30,  28,  30,  26,  18,  10,  // r6
      6,  14,  22,  26,  24,  26,  22,  14,   6,  // r7
      0,   6,  12,  16,  14,  16,  12,   6,   0,  // r8
     -6,  -2,   4,   8,   6,   8,   4,  -2,  -6   // r9
  ];

  const PST_CANNON = [
      0,   0,   2,   6,   6,   6,   2,   0,   0,  // r0
      0,   2,   4,   6,   6,   6,   4,   2,   0,  // r1
      4,   6,   8,  14,  20,  14,   8,   6,   4,  // r2 (starting cannon rank)
      0,   4,   6,  10,  14,  10,   6,   4,   0,  // r3
     -2,   2,   4,   8,  12,   8,   4,   2,  -2,  // r4
      0,   2,   6,  10,  14,  10,   6,   2,   0,  // r5
      2,   4,   8,  12,  16,  12,   8,   4,   2,  // r6
      4,   6,  10,  14,  18,  14,  10,   6,   4,  // r7
      6,  10,  14,  18,  22,  18,  14,  10,   6,  // r8
      6,  12,  16,  20,  24,  20,  16,  12,   6   // r9 (bottom rank cannon)
  ];

  const PST_CHARIOT = [
      0,   2,   4,   8,   8,   8,   4,   2,   0,  // r0
      4,   8,  10,  14,  14,  14,  10,   8,   4,  // r1
      4,   8,  10,  14,  14,  14,  10,   8,   4,  // r2
      6,  12,  14,  18,  18,  18,  14,  12,   6,  // r3
     12,  18,  22,  26,  26,  26,  22,  18,  12,  // r4 (river patrol)
     14,  20,  24,  28,  28,  28,  24,  20,  14,  // r5
     16,  22,  26,  30,  30,  30,  26,  22,  16,  // r6
     18,  24,  28,  34,  34,  34,  28,  24,  18,  // r7
     20,  26,  30,  36,  36,  36,  30,  26,  20,  // r8 (palace threat)
     10,  16,  20,  24,  24,  24,  20,  16,  10   // r9
  ];

  const PST_ADVISOR = [
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,  10,   0,  10,   0,   0,   0,
      0,   0,   0,   0,  20,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0
  ];

  const PST_ELEPHANT = [
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,  10,   0,   0,   0,  10,   0,   0,
      0,   0,   0,   0,  20,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
     10,   0,   0,   0,   0,   0,   0,   0,  10,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0
  ];

  const PST_KING = [
      0,   0,   0,   5,  10,   5,   0,   0,   0,
      0,   0,   0,   0,   4,   0,   0,   0,   0,
      0,   0,   0,  -4,  -2,  -4,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0,
      0,   0,   0,   0,   0,   0,   0,   0,   0
  ];

  // ───────────────────────────── Zobrist Hashing ─────────────────────────────
  const PIECE_INDEX = { k:0, a:1, e:2, n:3, r:4, c:5, p:6 };
  let ZOBRIST = null;

  function initZobrist() {
    if (ZOBRIST) return;
    let seed = 48271;
    function rand32() {
      seed = (seed * 1664525 + 1013904223) | 0;
      return seed >>> 0;
    }
    ZOBRIST = {
      pieces: Array.from({ length: NSQ }, () => 
        Array.from({ length: 14 }, () => [rand32(), rand32()])
      ),
      turn: [rand32(), rand32()]
    };
  }
  initZobrist();

  function computeHash(s) {
    let h0 = 0, h1 = 0;
    for (let sq = 0; sq < NSQ; sq++) {
      const p = s.b[sq];
      if (!p) continue;
      const idx = PIECE_INDEX[p.t] + (p.c === 'r' ? 0 : 7);
      const z = ZOBRIST.pieces[sq][idx];
      h0 ^= z[0]; h1 ^= z[1];
    }
    if (s.turn === 'b') {
      h0 ^= ZOBRIST.turn[0]; h1 ^= ZOBRIST.turn[1];
    }
    return { h0: h0 >>> 0, h1: h1 >>> 0 };
  }

  // ───────────────────────────── Transposition Table ─────────────────────────────
  const TT_SIZE = 65536;
  const TT_MASK = TT_SIZE - 1;
  const TT = new Array(TT_SIZE);

  // Scores depend on the entire path and draw counters; cache only ordering hints.
  function ttStore(h, bestMove) {
    TT[h.h0 & TT_MASK] = { h0:h.h0, h1:h.h1, bestMove };
  }

  function ttProbe(h) {
    const idx = h.h0 & TT_MASK;
    const entry = TT[idx];
    if (entry && entry.h0 === h.h0 && entry.h1 === h.h1) return entry;
    return null;
  }

  function ttClear() {
    TT.fill(null);
  }

  // ───────────────────────────── Positional Evaluation ─────────────────────────────
  function evaluate(s) {
    let score = 0;

    for (let sq = 0; sq < NSQ; sq++) {
      const p = s.b[sq];
      if (!p) continue;

      const isRed = p.c === 'r';
      const f = sq % NF, r = Math.floor(sq / NF);
      const pstSq = isRed ? sq : ((9 - r) * NF + f);
      let val = PIECE_VAL[p.t];

      switch (p.t) {
        case 'p':
          val += PST_SOLDIER_RED[pstSq];
          break;
        case 'n':
          val += PST_HORSE[pstSq];
          break;
        case 'c':
          val += PST_CANNON[pstSq];
          // Central cannon bonus (当头炮): cannon on center file 4
          if (f === 4) val += 30;
          break;
        case 'r':
          val += PST_CHARIOT[pstSq];
          // Open file bonus: check if file has any friendly pawns
          let pawnsOnFile = 0;
          for (let row = 0; row < NR; row++) {
            const piece = s.b[row * NF + f];
            if (piece && piece.t === 'p' && piece.c === p.c) pawnsOnFile++;
          }
          if (pawnsOnFile === 0) val += 20;
          break;
        case 'a':
          val += PST_ADVISOR[pstSq];
          break;
        case 'e':
          val += PST_ELEPHANT[pstSq];
          break;
        case 'k':
          val += PST_KING[pstSq];
          break;
      }

      if (isRed) {
        score += val;
      } else {
        score -= val;
      }
    }

    return s.turn === 'r' ? score : -score;
  }

  // ───────────────────────────── Move Ordering ─────────────────────────────
  const KILLERS = Array.from({ length: 64 }, () => [null, null]);
  const HISTORY = Array.from({ length: NSQ }, () => new Int32Array(NSQ));

  function clearHistory() {
    for (let i = 0; i < 64; i++) {
      KILLERS[i][0] = null;
      KILLERS[i][1] = null;
    }
    for (let i = 0; i < NSQ; i++) {
      HISTORY[i].fill(0);
    }
  }

  function scoreMove(m, ttMove, ply) {
    if (ttMove && m.from === ttMove.from && m.to === ttMove.to) {
      return 200000;
    }
    if (m.cap) {
      const vVal = PIECE_VAL[m.cap.t] || 100;
      const aVal = PIECE_VAL[m.piece.t] || 100;
      return 100000 + vVal * 10 - aVal;
    }
    if (ply < 64) {
      const k = KILLERS[ply];
      if (k[0] && m.from === k[0].from && m.to === k[0].to) return 80000;
      if (k[1] && m.from === k[1].from && m.to === k[1].to) return 70000;
    }
    return HISTORY[m.from][m.to] || 0;
  }

  // Deadline/recursion interruptions unwind before any incomplete iteration is published.
  const INTERRUPTED = Symbol('search interrupted');
  let nodesCount = 0, deadline = Infinity;
  let path = [];

  function quiescence(s, alpha, beta, ply, rules, moves, isCheck) {
    if (!isCheck) {
      const standPat = evaluate(s);
      if (standPat >= beta) return beta;
      alpha = Math.max(alpha, standPat);
      moves = moves.filter(m => m.cap || m.promo);
    }
    // In check every evasion is mandatory; no stand-pat or delta pruning.
    moves.sort((a, b) => scoreMove(b, null, ply) - scoreMove(a, null, ply));
    for (const m of moves) {
      const score = -negamax(rules.applyMove(s, m), 0, -beta, -alpha, ply + 1, rules);
      if (score >= beta) return beta;
      alpha = Math.max(alpha, score);
    }
    return alpha;
  }

  function negamax(s, depth, alpha, beta, ply, rules) {
    nodesCount++;
    if (Date.now() >= deadline) throw INTERRUPTED;
    path.push(rules.posKey(s));
    try {
      const moves = rules.legalMoves(s);
      const isCheck = rules.inCheck(s, s.turn);
      if (!moves.length) return -40000 + ply;
      if (rules.drawReason(s, path)) return 0;
      // shortcut: cap continuations at 96 plies, revisit if endgame fixtures require longer lines.
      if (ply >= 96) {
        if (isCheck) throw INTERRUPTED;
        return evaluate(s);
      }
      if (depth <= 0) return quiescence(s, alpha, beta, ply, rules, moves, isCheck);
      const hash = computeHash(s), tt = ttProbe(hash);
      moves.sort((a, b) => scoreMove(b, tt && tt.bestMove, ply) - scoreMove(a, tt && tt.bestMove, ply));
      let bestMove = moves[0];
      for (const m of moves) {
        const score = -negamax(rules.applyMove(s, m), depth - 1, -beta, -alpha, ply + 1, rules);
        if (score > alpha) { alpha = score; bestMove = m; }
        if (alpha >= beta) {
          if (!m.cap && ply < 64) {
            KILLERS[ply][1] = KILLERS[ply][0]; KILLERS[ply][0] = m;
            HISTORY[m.from][m.to] += depth * depth;
          }
          break;
        }
      }
      ttStore(hash, bestMove);
      return alpha;
    } finally { path.pop(); }
  }

  function search(state, level, rules, options = {}) {
    const t0 = Date.now();
    const configs = {
      1: { maxDepth:1, noise:120, timeLimit:200 },
      2: { maxDepth:3, noise:50, timeLimit:500 },
      3: { maxDepth:4, noise:15, timeLimit:1000 },
      4: { maxDepth:6, noise:0, timeLimit:1800 },
      5: { maxDepth:8, noise:0, timeLimit:2500 }
    };

    const cfg = configs[level] || configs[3];
    const timeLimit = options.timeLimit ?? cfg.timeLimit;
    deadline = t0 + Math.max(0, timeLimit);
    nodesCount = 0;
    clearHistory();
    // UI history includes the root; recursion appends each child and restores it in finally.
    path = (options.historyKeys || []).slice();
    const rootKey = rules.posKey(state);
    if (path[path.length - 1] !== rootKey) path.push(rootKey);
    const moves = rules.legalMoves(state);
    if (!moves.length || rules.drawReason(state, path)) return null;
    let bestMove = moves[0], bestScore = 0, completedDepth = 0;
    for (let d = 1; d <= cfg.maxDepth; d++) {
      let currentBestMove = null, alpha = -Infinity;
      moves.sort((a, b) => scoreMove(b, bestMove, 0) - scoreMove(a, bestMove, 0));
      try {
        for (const m of moves) {
          const score = -negamax(rules.applyMove(state, m), d - 1, -Infinity, -alpha, 1, rules);
          const noisy = score + (cfg.noise ? (Math.random() * 2 - 1) * cfg.noise : 0);
          if (noisy > alpha) { alpha = noisy; currentBestMove = m; }
        }
      } catch (e) {
        if (e !== INTERRUPTED) throw e;
        break;
      }
      bestMove = currentBestMove; bestScore = alpha; completedDepth = d;
      if (options.onProgress) options.onProgress({ depth:d, bestMove, score:bestScore, nodes:nodesCount, timeMs:Date.now()-t0 });
      if (Math.abs(bestScore) > 35000 || Date.now() >= deadline) break;
    }
    return { move:bestMove, score:bestScore, depth:completedDepth, nodes:nodesCount, timeMs:Date.now()-t0 };
  }

  if (typeof WorkerGlobalScope !== 'undefined' && root instanceof WorkerGlobalScope) {
    root.onmessage = ({ data }) => {
      if (data.type !== 'search') return;
      const { id, state, level, timeLimit, historyKeys } = data;
      try {
        const result = search(state, level, root.GameRules, { timeLimit, historyKeys,
          onProgress:p => root.postMessage({ type:'progress', id, ...p }) });
        root.postMessage({ type:'result', id, result });
      } catch (e) { root.postMessage({ type:'error', id, message:String(e.message || e) }); }
    };
  }

  // ───────────────────────────── Module Export ─────────────────────────────
  const XiangqiAI = {
    workerSource: '(' + installAI.toString() + ')(self);',
    search,
    evaluate,
    computeHash,
    ttClear,
    initZobrist
  };

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = XiangqiAI;
  } else {
    root.XiangqiAI = XiangqiAI;
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
