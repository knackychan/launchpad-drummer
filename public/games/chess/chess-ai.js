/**
 * chess-ai.js - High-Performance Multi-Level Chess AI Engine for LaunchBand
 * 
 * Features:
 * - Dual-phase PeSTO Piece-Square Tables (Middlegame + Endgame interpolation)
 * - 64-bit Zobrist move-ordering cache (no history-dependent score reuse)
 * - Quiescence Search with MVV-LVA capture ordering
 * - Killer Move Heuristic (2 killers per ply) & History Heuristic (64x64)
 * - Iterative Deepening Negamax with Alpha-Beta pruning
 * - 5 difficulty settings (unrated)
 * - Usable in Browser (Main Thread & Web Worker) and Node.js
 */
(function installAI(root) {
  'use strict';

  // ───────────────────────────── Evaluation Tables (PeSTO's Dual-Phase) ─────────────────────────────
  const PIECE_VAL_MG = { p: 82,  n: 337, b: 365, r: 477, q: 1025, k: 0 };
  const PIECE_VAL_EG = { p: 94,  n: 281, b: 297, r: 512, q: 936,  k: 0 };
  const PHASE_WEIGHT = { p: 0,   n: 1,   b: 1,   r: 2,   q: 4,    k: 0 };

  // Piece square tables run from a8..h8 to a1..h1 for White.
  const PST_MG = {
    p: [
        0,   0,   0,   0,   0,   0,   0,   0,
       98, 134,  61,  95,  68, 126,  34, -11,
       -6,   7,  26,  31,  65,  56,  25, -20,
      -14,  13,   6,  21,  23,  12,  17, -23,
      -27,  -2,  -5,  12,  17,   6,  10, -25,
      -26,  -4,  -4, -10,   3,   3,  33, -12,
      -35,  -1, -20, -23, -15,  24,  38, -22,
        0,   0,   0,   0,   0,   0,   0,   0
    ],
    n: [
      -167, -89, -34, -49,  61, -97, -15, -107,
       -73, -41,  72,  36,  23,  62,   7,  -17,
       -47,  60,  37,  65,  84, 129,  73,   44,
        -9,  17,  19,  53,  37,  69,  18,   22,
       -13,   4,  16,  13,  28,  19,  21,   -8,
       -23,  -9,  12,  10,  19,  17,  25,  -16,
       -29, -53, -12,  -3,  -1,  18, -14,  -19,
      -105, -21, -58, -33, -17, -28, -19,  -23
    ],
    b: [
       -29,   4, -82, -37, -25, -42,   7,  -8,
       -26,  16, -18, -13,  30,  59,  18, -47,
       -16,  37,  43,  40,  35,  50,  37,  -2,
        -4,   5,  19,  50,  37,  37,   7,  -2,
        -6,  13,  13,  26,  34,  12,  10,   4,
         0,  15,  15,  15,  14,  27,  18,  10,
         4,  15,  16,   0,   7,  21,  33,   1,
       -33,  -3, -14, -21, -13, -12, -39, -21
    ],
    r: [
        32,  42,  32,  51,  63,   9,  31,  43,
        27,  32,  58,  62,  80,  67,  26,  44,
        -5,  19,  26,  36,  17,  45,  61,  16,
       -24, -11,   7,  26,  24,  35,  -8, -20,
       -36, -26, -12,  -1,   9,  -7,   6, -23,
       -45, -25, -16, -17,   3,   0,  -5, -33,
       -44, -16, -20,  -9,  -1,  11,  -6, -71,
       -19, -13,   1,  17,  16,   7, -37, -26
    ],
    q: [
       -28,   0,  29,  12,  59,  44,  43,  45,
       -24, -39,  -5,   1, -16,  57,  28,  54,
       -13, -17,   7,   8,  29,  56,  47,  57,
       -27, -27, -16, -16,  -1,  17,  -2,   1,
        -9, -26,  -9, -10,  -2,  -4,   3,  -3,
       -14,   2, -11,  -2,  -5,   2,  14,   5,
       -35,  -8,  11,   2,   8,  15,  -3,   1,
        -1, -18,  -9,  10, -15, -25, -31, -50
    ],
    k: [
       -65,  23,  16, -15, -56, -34,   2,  13,
        29,  -1, -20,  -7,  -8,  -4, -38, -29,
        -9,  24,   2, -16, -20,   6,  22, -22,
       -17, -20, -12, -27, -30, -25, -14, -36,
       -49,  -1, -27, -39, -46, -44, -33, -51,
       -14, -14, -22, -46, -44, -30, -15, -27,
         1,   7,  -8, -64, -43, -16,   9,   8,
       -15,  36,  12, -54,   8, -28,  24,  14
    ]
  };

  const PST_EG = {
    p: [
        0,   0,   0,   0,   0,   0,   0,   0,
      178, 173, 158, 134, 147, 132, 165, 187,
       94, 100,  85,  67,  56,  53,  82,  84,
       32,  24,  13,   5,  -2,   4,  17,  17,
       13,   9,  -3,  -7,  -7,  -8,   3,  -1,
        4,   7,  -6,   1,   0,  -5,  -1,  -8,
       13,   8,   8, -10,  -7,   0,  15,  -7,
        0,   0,   0,   0,   0,   0,   0,   0
    ],
    n: [
      -58, -38, -13, -28, -31, -27, -63, -99,
      -25,  -8, -25,  -2,  -9, -25, -24, -52,
      -24, -20,  10,   9,  -1,  -9, -19, -41,
      -17,   3,  22,  22,  22,  11,   8, -18,
      -18,  -6,  16,  25,  16,  17,   4, -18,
      -23,  -3,  -1,  15,  10,  -3, -20, -22,
      -42, -20, -10,  -5,  -2, -20, -23, -44,
      -29, -51, -23, -15, -22, -18, -50, -64
    ],
    b: [
      -14, -21, -11,  -8,  -7,  -9, -17, -24,
       -8,  -4,   7, -12,  -3, -13,  -4, -14,
        2,  -8,   0,  -1,  -2,   6,   0,   4,
       -3,   9,  12,   9,  14,  10,   3,   2,
       -6,   3,  13,  19,   7,  10,  -3,  -9,
      -12,  -3,   8,  10,  13,   3,  -7, -15,
      -14, -18,  -7,  -1,   4,  -9, -15, -27,
      -23,  -9, -23,  -5,  -9, -16,  -5, -17
    ],
    r: [
       13,  10,  18,  15,  12,  12,   8,   5,
       11,  13,  13,  11,  -3,   3,   8,   3,
        7,   7,   7,   5,   4,  -3,  -5,  -3,
        4,   3,  13,   1,   2,   1,  -1,   2,
        3,   5,   8,   4,  -5,  -6,  -8, -11,
       -4,   0,  -5,  -1,  -7, -12,  -8, -16,
       -6,  -6,   0,   2,  -9,  -9, -11,  -3,
       -9,   2,   3,  -1,  -5, -13,   4, -20
    ],
    q: [
       -9,  22,  22,  27,  27,  19,  10,  20,
      -17,  20,  32,  41,  58,  25,  30,   0,
      -20,   6,   9,  49,  47,  35,  19,   9,
        3,  22,  24,  45,  57,  40,  57,  36,
      -18,  28,  19,  47,  31,  34,  39,  18,
      -16, -27,  15,   6,   9,  17,  10,   5,
      -22, -23, -30, -16, -16, -23, -36, -32,
      -33, -28, -22, -43,  -5, -32, -20, -41
    ],
    k: [
      -74, -35, -18, -18, -11,  15,   4, -17,
      -12,  17,  14,  17,  17,  38,  23,  11,
       10,  17,  23,  15,  20,  45,  44,  13,
       -8,  22,  24,  27,  26,  33,  26,   3,
      -18,  -4,  21,  24,  27,  23,   9, -11,
      -19,  -3,  11,  21,  23,  16,   7,  -9,
      -27, -11,   4,  13,  14,   4,  -5, -17,
      -53, -34, -21, -11, -28, -14, -24, -43
    ]
  };

  // ───────────────────────────── Zobrist Hashing ─────────────────────────────
  const PIECE_INDEX = { p:0, n:1, b:2, r:3, q:4, k:5 };
  let ZOBRIST = null;

  function initZobrist() {
    if (ZOBRIST) return;
    // PRNG for consistent keys
    let seed = 1070372;
    function rand32() {
      seed = (seed * 1664525 + 1013904223) | 0;
      return seed >>> 0;
    }
    ZOBRIST = {
      pieces: Array.from({ length: 64 }, () => 
        Array.from({ length: 12 }, () => [rand32(), rand32()])
      ),
      turn: [rand32(), rand32()],
      castle: Array.from({ length: 16 }, () => [rand32(), rand32()]),
      ep: Array.from({ length: 9 }, () => [rand32(), rand32()])
    };
  }
  initZobrist();

  function computeHash(s) {
    let h0 = 0, h1 = 0;
    for (let sq = 0; sq < 64; sq++) {
      const p = s.b[sq];
      if (!p) continue;
      const idx = PIECE_INDEX[p.t] + (p.c === 'w' ? 0 : 6);
      const z = ZOBRIST.pieces[sq][idx];
      h0 ^= z[0]; h1 ^= z[1];
    }
    if (s.turn === 'b') {
      h0 ^= ZOBRIST.turn[0]; h1 ^= ZOBRIST.turn[1];
    }
    const cIdx = (s.castle.wK ? 1 : 0) | (s.castle.wQ ? 2 : 0) | (s.castle.bK ? 4 : 0) | (s.castle.bQ ? 8 : 0);
    h0 ^= ZOBRIST.castle[cIdx][0]; h1 ^= ZOBRIST.castle[cIdx][1];
    const epIdx = s.ep >= 0 ? (s.ep & 7) + 1 : 0;
    h0 ^= ZOBRIST.ep[epIdx][0]; h1 ^= ZOBRIST.ep[epIdx][1];
    return { h0: h0 >>> 0, h1: h1 >>> 0 };
  }

  // ───────────────────────────── Transposition Table ─────────────────────────────
  const TT_SIZE = 65536; // 64K entries
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

  // ───────────────────────────── Evaluation Function ─────────────────────────────
  function evaluate(s) {
    let mgScore = 0, egScore = 0, gamePhase = 0;

    for (let sq = 0; sq < 64; sq++) {
      const p = s.b[sq];
      if (!p) continue;
      const isW = p.c === 'w';
      const t = p.t;
      // Tables run opposite to board ranks: flip White, index Black directly.
      const r = sq >> 3, f = sq & 7;
      const pstSq = isW ? ((7 - r) * 8 + f) : sq;

      const mgVal = PIECE_VAL_MG[t] + PST_MG[t][pstSq];
      const egVal = PIECE_VAL_EG[t] + PST_EG[t][pstSq];
      gamePhase += PHASE_WEIGHT[t];

      if (isW) {
        mgScore += mgVal;
        egScore += egVal;
      } else {
        mgScore -= mgVal;
        egScore -= egVal;
      }
    }

    // Phase scaling: 0 (pure endgame) to 24 (opening)
    const mgWeight = Math.min(24, gamePhase);
    const egWeight = 24 - mgWeight;
    const score = Math.round((mgScore * mgWeight + egScore * egWeight) / 24);

    return s.turn === 'w' ? score : -score;
  }

  // ───────────────────────────── Move Ordering ─────────────────────────────
  const KILLERS = Array.from({ length: 64 }, () => [null, null]);
  const HISTORY = Array.from({ length: 64 }, () => new Int32Array(64));

  function clearHistory() {
    for (let i = 0; i < 64; i++) {
      KILLERS[i][0] = null;
      KILLERS[i][1] = null;
      HISTORY[i].fill(0);
    }
  }

  function scoreMove(m, ttMove, ply) {
    // 1. TT move
    if (ttMove && m.from === ttMove.from && m.to === ttMove.to && m.promo === ttMove.promo) {
      return 200000;
    }
    // 2. Captures MVV-LVA
    if (m.cap) {
      const vVal = PIECE_VAL_MG[m.cap.t] || 100;
      const aVal = PIECE_VAL_MG[m.piece.t] || 100;
      return 100000 + vVal * 10 - aVal;
    }
    // 3. Promotions
    if (m.promo) {
      return 90000 + (PIECE_VAL_MG[m.promo] || 0);
    }
    // 4. Killer moves
    if (ply < 64) {
      const k = KILLERS[ply];
      if (k[0] && m.from === k[0].from && m.to === k[0].to) return 80000;
      if (k[1] && m.from === k[1].from && m.to === k[1].to) return 70000;
    }
    // 5. History heuristic
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
      if (!moves.length) return isCheck ? -40000 + ply : 0;
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
  const ChessAI = {
    workerSource: '(' + installAI.toString() + ')(self);',
    search,
    evaluate,
    computeHash,
    ttClear,
    initZobrist
  };

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = ChessAI;
  } else {
    root.ChessAI = ChessAI;
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
