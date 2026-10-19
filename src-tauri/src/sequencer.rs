//! Step sequencer clock (plan 1C, Task 13).
//!
//! The router owns the pattern (in the set); this module owns the clock state, the pure timing
//! rules around it, and the thread that fires steps. Deviation D3: the clock sleeps until 1 ms
//! before each step and spins for the rest, so the jitter is about 1 ms; it is not
//! sample-accurate as UI spec 4.7 asked, because the triggers go through the same command queue
//! as every other pad hit.
//!
//! Locks: `seq` is a leaf below the router (the record hook takes it while the router is held).
//! The clock thread never holds `seq` while it takes the router.

use crate::state::AppState;
use lpd_core::seq::{
    add_lane, due_steps, loop_duration, nearest_step, step_duration, step_time, Pattern, MAX_STEPS,
};
use serde::Serialize;
use std::collections::HashSet;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// The position goes out at least this often while playing.
const POSITION_INTERVAL: Duration = Duration::from_millis(33);
/// The clock sleeps until this long before a step and spins for the rest.
const SPIN: Duration = Duration::from_millis(1);
/// A held pad is released after this share of a step.
const HOLD_SHARE: f64 = 0.9;

/// What the clock needs from the pattern, clamped to the legal ranges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub length: usize,
    pub bpm: f32,
    pub swing: f32,
}

impl From<&Pattern> for Params {
    fn from(p: &Pattern) -> Self {
        Params {
            length: usize::from(p.length).clamp(1, MAX_STEPS),
            bpm: p.bpm.clamp(60.0, 200.0),
            swing: p.swing.clamp(50.0, 75.0),
        }
    }
}

impl Params {
    fn step_time(&self, i: usize) -> f64 {
        step_time(i, self.bpm, self.swing)
    }

    fn loop_len(&self) -> f64 {
        loop_duration(self.length, self.bpm)
    }

    /// The step running at loop time `t`, and how far into it we are (seconds).
    fn step_at(&self, t: f64) -> (usize, f64) {
        let i = (0..self.length)
            .rev()
            .find(|&s| self.step_time(s) <= t)
            .unwrap_or(0);
        (i, (t - self.step_time(i)).max(0.0))
    }
}

/// `seq-position` payload (plan 0.4).
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SeqPosition {
    pub playing: bool,
    pub rec: bool,
    pub step: usize,
    pub length: usize,
    pub bpm: f32,
    pub ms_into_step: f64,
}

pub struct SeqState {
    pub playing: bool,
    pub rec: bool,
    /// Loop start reference; the clock folds it forward by whole loops.
    pub started: Instant,
    /// Loop time up to which steps have fired.
    pub last_t: f64,
    pub params: Params,
    /// `(lane, step)` pairs that were played live while recording: the clock must not fire them
    /// on this pass.
    pub skip: HashSet<(usize, usize)>,
    /// Held pads to release: `(when, bank, x, y)`.
    pub releases: Vec<(Instant, usize, u8, u8)>,
    /// Recording changed the pattern: the clock thread tells the UI and marks the set dirty.
    pub dirty: bool,
}

impl Default for SeqState {
    fn default() -> Self {
        Self::new()
    }
}

impl SeqState {
    pub fn new() -> Self {
        SeqState {
            playing: false,
            rec: false,
            started: Instant::now(),
            last_t: 0.0,
            params: Params::from(&Pattern::default()),
            skip: HashSet::new(),
            releases: Vec::new(),
            dirty: false,
        }
    }

    /// Play from step 0; `last_t` just below 0 so step 0 fires at once.
    pub fn start(&mut self, now: Instant, params: Params) {
        self.playing = true;
        self.started = now;
        self.last_t = -1e-9;
        self.params = params;
        self.skip.clear();
    }

    /// Stop the clock (REC stays armed). Pending releases still happen.
    pub fn stop(&mut self) {
        self.playing = false;
        self.skip.clear();
    }

    /// Loop time of `now`, in `[0, loop)`.
    pub fn loop_time(&self, now: Instant, params: &Params) -> f64 {
        now.saturating_duration_since(self.started)
            .as_secs_f64()
            .rem_euclid(params.loop_len())
    }

    /// Advance the clock to `now` and return the steps that came due, in order. Edits of tempo,
    /// swing and length take effect after this tick's steps (with the old settings): the
    /// reference is moved so the current step and its progress are kept.
    pub fn tick(&mut self, now: Instant, params: Params) -> Vec<usize> {
        if !self.playing {
            return Vec::new();
        }
        let old = self.params;
        let mut t = now.saturating_duration_since(self.started).as_secs_f64();
        let due = due_steps(self.last_t, t, old.length, old.bpm, old.swing);
        let l = old.loop_len();
        if t >= l {
            let folds = (t / l).floor();
            self.started += Duration::from_secs_f64(folds * l);
            t -= folds * l;
        }
        self.last_t = t;
        if params != old {
            let (step, into) = old.step_at(t);
            let progress = into / step_duration(old.bpm);
            let step = step % params.length;
            let t_new = params.step_time(step) + progress * step_duration(params.bpm);
            self.started = now - Duration::from_secs_f64(t_new.max(0.0));
            self.last_t = t_new;
            self.params = params;
        }
        due
    }

    /// When the next step is due.
    pub fn next_step_at(&self) -> Instant {
        let p = &self.params;
        let next = (0..p.length)
            .map(|i| p.step_time(i))
            .find(|&t| t > self.last_t)
            .unwrap_or_else(|| p.loop_len());
        self.started + Duration::from_secs_f64(next.max(0.0))
    }

    pub fn position(&self, now: Instant) -> SeqPosition {
        let p = &self.params;
        let (step, into) = if self.playing {
            p.step_at(self.loop_time(now, p))
        } else {
            (0, 0.0)
        };
        SeqPosition {
            playing: self.playing,
            rec: self.rec,
            step,
            length: p.length,
            bpm: p.bpm,
            ms_into_step: into * 1000.0,
        }
    }

    /// Record rule (UI spec 4.7) for a hit on `(bank, x, y)` that did not come from the clock.
    /// Only while playing with REC armed. Never lowers or removes a step. Returns whether the
    /// pattern changed.
    pub fn record(
        &mut self,
        pattern: &mut Pattern,
        bank: usize,
        x: u8,
        y: u8,
        now: Instant,
    ) -> bool {
        if !(self.playing && self.rec) {
            return false;
        }
        let p = Params::from(&*pattern);
        let t = self.loop_time(now, &p);
        let (step, already) = nearest_step(t, p.length, p.bpm, p.swing);
        let at = (bank as u8, x, y);
        let lane = match pattern.lanes.iter().position(|l| (l.bank, l.x, l.y) == at) {
            Some(i) => i,
            None => {
                if add_lane(pattern, at.0, at.1, at.2).is_err() {
                    return false;
                }
                pattern.lanes.len() - 1
            }
        };
        let steps = &mut pattern.lanes[lane].steps;
        steps.resize(MAX_STEPS, 0);
        steps[step] = steps[step].max(1);
        if already {
            self.skip.insert((lane, step));
        }
        self.dirty = true;
        true
    }
}

pub fn start(app: AppHandle, state: AppState) {
    std::thread::spawn(move || run(app, state));
}

fn emit_position(app: &AppHandle, position: &SeqPosition) {
    if let Err(error) = app.emit("seq-position", position.clone()) {
        crate::logfile::log(format!("seq-position event failed: {error}"));
    }
}

fn run(app: AppHandle, state: AppState) {
    let seq = state.seq_handle();
    let mut was_playing = false;
    let mut was_rec = false;
    let mut last_emit = Instant::now();
    loop {
        let now = Instant::now();
        let playing = seq.lock().unwrap_or_else(PoisonError::into_inner).playing;
        let mut due = Vec::new();
        let mut skip = HashSet::new();
        if playing {
            // The router lock is taken and released before the sequencer state is.
            let params = state.pattern_params();
            let mut s = seq.lock().unwrap_or_else(PoisonError::into_inner);
            due = s.tick(now, params);
            if !due.is_empty() {
                skip = s.skip.clone();
                s.skip.retain(|&(_, step)| !due.contains(&step));
            }
        }
        if !due.is_empty() {
            let step_len = {
                let s = seq.lock().unwrap_or_else(PoisonError::into_inner);
                step_duration(s.params.bpm)
            };
            let release_at = now + Duration::from_secs_f64(step_len * HOLD_SHARE);
            let fired = state.seq_fire(&due, &skip);
            let mut s = seq.lock().unwrap_or_else(PoisonError::into_inner);
            for pad in fired.into_iter().filter(|f| f.hold) {
                s.releases.push((release_at, pad.bank, pad.x, pad.y));
            }
        }
        let (releases, dirty) = {
            let mut s = seq.lock().unwrap_or_else(PoisonError::into_inner);
            let (due_now, later): (Vec<_>, Vec<_>) = s.releases.drain(..).partition(|r| r.0 <= now);
            s.releases = later;
            (due_now, std::mem::take(&mut s.dirty))
        };
        for (_, bank, x, y) in releases {
            state.seq_release(bank, x, y);
        }
        if dirty {
            state.mark_set_dirty();
        }

        let position = {
            let s = seq.lock().unwrap_or_else(PoisonError::into_inner);
            s.position(now)
        };
        if playing && (!due.is_empty() || now.duration_since(last_emit) >= POSITION_INTERVAL) {
            emit_position(&app, &position);
            last_emit = now;
        } else if (was_playing && !playing) || position.rec != was_rec {
            // Stopped, or REC changed while the clock was not running: the screen follows.
            emit_position(&app, &position);
        }
        was_playing = playing;
        was_rec = position.rec;

        if playing {
            let next_step = seq
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .next_step_at();
            let target = next_step.min(last_emit + POSITION_INTERVAL);
            if let Some(wait) = target.checked_duration_since(Instant::now()) {
                if wait > SPIN {
                    std::thread::sleep(wait - SPIN);
                }
            }
            while Instant::now() < target {
                std::hint::spin_loop();
            }
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

/// A pad the clock just triggered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fired {
    pub bank: usize,
    pub x: u8,
    pub y: u8,
    pub hold: bool,
}

/// Wrap a `Mutex<SeqState>` guard recovery.
pub fn lock(m: &Mutex<SeqState>) -> std::sync::MutexGuard<'_, SeqState> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(length: usize, bpm: f32, swing: f32) -> Params {
        Params { length, bpm, swing }
    }

    fn t0() -> Instant {
        Instant::now()
    }

    #[test]
    fn step_zero_fires_at_once_and_each_step_once() {
        let p = params(16, 120.0, 50.0);
        let t = t0();
        let mut s = SeqState::new();
        s.start(t, p);
        assert_eq!(s.tick(t, p), vec![0]);
        assert_eq!(
            s.tick(t + Duration::from_millis(100), p),
            Vec::<usize>::new()
        );
        assert_eq!(s.tick(t + Duration::from_millis(130), p), vec![1]);
        assert_eq!(
            s.tick(t + Duration::from_millis(130), p),
            Vec::<usize>::new()
        );
        // A late tick catches up on everything in between, in order.
        assert_eq!(s.tick(t + Duration::from_millis(400), p), vec![2, 3]);
    }

    #[test]
    fn the_loop_wraps_without_dropping_or_doubling_a_step() {
        let p = params(4, 120.0, 50.0); // loop = 0.5 s
        let t = t0();
        let mut s = SeqState::new();
        s.start(t, p);
        let mut fired = s.tick(t, p);
        for ms in (10..=1100).step_by(10) {
            fired.extend(s.tick(t + Duration::from_millis(ms), p));
        }
        // 1.1 s at 125 ms per step: steps at 0, 125, ... 1000 ms = 9 steps.
        assert_eq!(fired, vec![0, 1, 2, 3, 0, 1, 2, 3, 0]);
    }

    #[test]
    fn a_stopped_clock_fires_nothing() {
        let p = params(16, 120.0, 50.0);
        let mut s = SeqState::new();
        assert!(s.tick(t0(), p).is_empty());
        s.start(t0(), p);
        s.stop();
        assert!(s.tick(t0() + Duration::from_secs(1), p).is_empty());
    }

    #[test]
    fn a_tempo_change_keeps_the_step_and_its_progress() {
        let slow = params(16, 120.0, 50.0); // 125 ms per step
        let fast = params(16, 240.0, 50.0); // 62.5 ms per step
        let t = t0();
        let mut s = SeqState::new();
        s.start(t, slow);
        s.tick(t, slow);
        // 375 ms: step 3 has just started; 31 ms later, a quarter of the way into it.
        s.tick(t + Duration::from_millis(375), slow);
        let at = t + Duration::from_millis(375 + 31);
        let due = s.tick(at, fast);
        assert!(due.is_empty());
        let p = s.position(at);
        assert_eq!(p.step, 3);
        assert!((p.ms_into_step - 15.6).abs() < 3.0, "{}", p.ms_into_step);
        assert_eq!(p.bpm, 240.0);
        // The next step comes after the rest of the new step, not the old one.
        let next = s.next_step_at().duration_since(at).as_secs_f64();
        assert!((next - 0.047).abs() < 0.004, "{next}");
    }

    #[test]
    fn shrinking_the_length_wraps_the_current_step() {
        let long = params(16, 120.0, 50.0);
        let short = params(4, 120.0, 50.0);
        let t = t0();
        let mut s = SeqState::new();
        s.start(t, long);
        s.tick(t, long);
        s.tick(t + Duration::from_millis(1000), long); // step 8
        let at = t + Duration::from_millis(1010);
        s.tick(at, short);
        assert_eq!(s.position(at).step, 0, "8 % 4");
        assert_eq!(s.position(at).length, 4);
    }

    #[test]
    fn position_reports_step_and_time_into_it() {
        let p = params(16, 120.0, 50.0);
        let t = t0();
        let mut s = SeqState::new();
        s.start(t, p);
        s.tick(t, p);
        s.tick(t + Duration::from_millis(300), p);
        let pos = s.position(t + Duration::from_millis(300));
        assert_eq!(pos.step, 2);
        assert!((pos.ms_into_step - 50.0).abs() < 1.0);
        assert!(pos.playing && !pos.rec);
        assert_eq!(pos.length, 16);
    }

    fn pattern_with_lane() -> Pattern {
        let mut p = Pattern::default();
        p.bpm = 120.0;
        p
    }

    #[test]
    fn recording_needs_play_and_rec() {
        let t = t0();
        let mut s = SeqState::new();
        let mut p = pattern_with_lane();
        assert!(!s.record(&mut p, 0, 1, 1, t));
        s.start(t, Params::from(&p));
        assert!(!s.record(&mut p, 0, 1, 1, t), "playing but REC not armed");
        assert!(p.lanes.is_empty());
        s.rec = true;
        assert!(s.record(&mut p, 0, 1, 1, t + Duration::from_millis(10)));
        assert_eq!(p.lanes.len(), 1);
        assert_eq!(p.lanes[0].steps[0], 1);
        assert!(s.dirty);
    }

    #[test]
    fn a_late_hit_lands_on_the_next_step_and_is_not_fired_twice() {
        let t = t0();
        let mut s = SeqState::new();
        let mut p = pattern_with_lane();
        s.start(t, Params::from(&p));
        s.rec = true;
        // 70 ms into step 0 (125 ms steps): closer to step 1 → step 1, already played live.
        assert!(s.record(&mut p, 0, 2, 3, t + Duration::from_millis(70)));
        assert_eq!(p.lanes[0].steps[1], 1);
        assert!(s.skip.contains(&(0, 1)));
        // 10 ms after step 1 started: step 1, the clock already fired it: no skip.
        let mut q = pattern_with_lane();
        let mut s2 = SeqState::new();
        s2.start(t, Params::from(&q));
        s2.rec = true;
        s2.record(&mut q, 0, 2, 3, t + Duration::from_millis(135));
        assert_eq!(q.lanes[0].steps[1], 1);
        assert!(s2.skip.is_empty());
    }

    #[test]
    fn recording_never_lowers_an_accent_and_reuses_the_lane() {
        let t = t0();
        let mut s = SeqState::new();
        let mut p = pattern_with_lane();
        add_lane(&mut p, 1, 4, 4).unwrap();
        p.lanes[0].steps[0] = 2;
        s.start(t, Params::from(&p));
        s.rec = true;
        s.record(&mut p, 1, 4, 4, t + Duration::from_millis(5));
        assert_eq!(p.lanes.len(), 1);
        assert_eq!(p.lanes[0].steps[0], 2);
    }

    #[test]
    fn the_loop_end_wraps_the_recorded_step_to_zero() {
        let t = t0();
        let mut s = SeqState::new();
        let mut p = pattern_with_lane();
        s.start(t, Params::from(&p));
        s.rec = true;
        s.record(&mut p, 0, 0, 0, t + Duration::from_millis(1990));
        assert_eq!(p.lanes[0].steps[0], 1);
        assert!(s.skip.contains(&(0, 0)));
    }

    #[test]
    fn params_are_clamped() {
        let mut p = Pattern::default();
        p.bpm = 5.0;
        p.swing = 99.0;
        p.length = 0;
        let q = Params::from(&p);
        assert_eq!((q.bpm, q.swing, q.length), (60.0, 75.0, 1));
    }
}
