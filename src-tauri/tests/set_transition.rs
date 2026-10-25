//! Findings 2 and 3: replacing a set stops the old sound, frees old samples off the audio
//! thread, never loses the stop, and never overwrites an existing kit.

mod common;

use common::{drain, rig, write_wav};
use launchpad_drummer_lib::state::AppState;
use lpd_core::audio::{host, Command, Engine};
use lpd_core::kit::PadMode;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::path::Path;
use std::sync::Arc;

struct Counting;

thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
    static FREES: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK.with(Cell::get) {
            ALLOCS.with(|c| c.set(c.get() + 1));
        }
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if TRACK.with(Cell::get) {
            FREES.with(|c| c.set(c.get() + 1));
        }
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if TRACK.with(Cell::get) {
            ALLOCS.with(|c| c.set(c.get() + 1));
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// (allocations, deallocations) made by `f` on this thread.
fn counted<T>(f: impl FnOnce() -> T) -> (T, usize, usize) {
    ALLOCS.with(|c| c.set(0));
    FREES.with(|c| c.set(0));
    TRACK.with(|t| t.set(true));
    let out = f();
    TRACK.with(|t| t.set(false));
    (out, ALLOCS.with(Cell::get), FREES.with(Cell::get))
}

struct Bench {
    state: AppState,
    engine: Engine,
    commands: rtrb::Consumer<Command>,
    scratch: Vec<f32>,
    out: Vec<f32>,
}

impl Bench {
    fn render_blocks(&mut self, blocks: usize) -> (Vec<f32>, usize, usize) {
        let ((), allocs, frees) = counted(|| {
            for _ in 0..blocks {
                host::process(
                    &mut self.engine,
                    &mut self.commands,
                    &mut self.scratch,
                    &mut self.out,
                    2,
                );
            }
        });
        (self.out.clone(), allocs, frees)
    }
}

/// A real engine behind an AppState whose open set holds one long hold-mode pad at (0, 0).
fn bench_with_playing_pad(dir: &Path) -> (Bench, std::sync::Weak<lpd_core::audio::Sample>) {
    let (engine, events) = Engine::new(48_000);
    let (tx, commands) = rtrb::RingBuffer::new(256);
    let state = AppState::new(lpd_core::kit::Set::new(), tx, events);
    let set_dir = dir.join("kit");
    state.new_set(set_dir.to_str().unwrap()).unwrap();
    let wav = dir.join("long.wav");
    write_wav(&wav, 96_000);
    state
        .assign_sample(0, 0, 0, wav.to_str().unwrap(), Some([60, 0, 0]))
        .unwrap();
    let mut pad = state.get_current_set().banks[0].pad(0, 0).unwrap().clone();
    pad.mode = PadMode::Hold;
    state.update_pad(0, pad, None);
    let sample = state
        .kit_manager
        .lock()
        .unwrap()
        .samples
        .get("samples/long.wav")
        .unwrap()
        .clone();
    let weak = Arc::downgrade(&sample);
    drop(sample);
    (
        Bench {
            state,
            engine,
            commands,
            scratch: host::scratch(),
            out: vec![0.0; 512 * 2],
        },
        weak,
    )
}

#[test]
fn switching_sets_fades_old_sound_and_frees_samples_outside_rendering() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut bench, sample) = bench_with_playing_pad(tmp.path());

    bench.state.trigger_pad(0, 0);
    let (out, allocs, frees) = bench.render_blocks(4);
    assert!(out.iter().any(|s| *s != 0.0), "the hold pad sounds");
    assert_eq!((allocs, frees), (0, 0));
    bench.state.maintain();
    assert!(bench.state.router.lock().unwrap().is_pad_sounding(0, 0, 0));

    // Switch sets while the pad is held, then release the original pad.
    let other = tmp.path().join("other");
    bench.state.new_set(other.to_str().unwrap()).unwrap();
    bench.state.release_pad(0, 0);

    // Render through the fade and long after. Neither allocation nor sample destruction may
    // happen in rendering, even though the voice held the last external reference.
    let (out, allocs, frees) = bench.render_blocks(8);
    assert_eq!(
        (allocs, frees),
        (0, 0),
        "no allocation or free while rendering"
    );
    assert!(out.iter().all(|s| *s == 0.0), "the old sound has faded out");
    assert!(
        sample.upgrade().is_some(),
        "the retired sample is still alive: nothing freed it on the audio side"
    );

    // The worker side reclaims it, now that the voice is gone.
    let ((), _, frees) = counted(|| bench.state.maintain());
    assert!(frees > 0, "the buffer was freed by the housekeeping worker");
    assert!(sample.upgrade().is_none());
    assert!(!bench.state.router.lock().unwrap().is_pad_sounding(0, 0, 0));
}

#[test]
fn a_sample_still_playing_is_not_freed_early() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut bench, sample) = bench_with_playing_pad(tmp.path());
    bench.state.trigger_pad(0, 0);
    bench.render_blocks(1);
    let other = tmp.path().join("other");
    bench.state.new_set(other.to_str().unwrap()).unwrap();

    // The stop has not been rendered yet: the voice still owns the sample.
    bench.state.maintain();
    assert!(sample.upgrade().is_some());
    assert_eq!(bench.state.router.lock().unwrap().retired_count(), 1);

    bench.render_blocks(4);
    bench.state.maintain();
    assert!(sample.upgrade().is_none());
    assert_eq!(bench.state.router.lock().unwrap().retired_count(), 0);
}

#[test]
fn old_voice_events_cannot_leave_the_new_pad_sounding() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut bench, _sample) = bench_with_playing_pad(tmp.path());
    bench.state.trigger_pad(0, 0);
    bench.render_blocks(1); // VoiceStarted is in flight, not yet polled
    let other = tmp.path().join("other");
    bench.state.new_set(other.to_str().unwrap()).unwrap();
    bench.render_blocks(4); // the stop fade ends the voice: VoiceEnded in flight too
    bench.state.maintain();
    assert!(
        !bench.state.router.lock().unwrap().is_pad_sounding(0, 0, 0),
        "start and end both counted, so the counter is balanced"
    );
}

#[test]
fn the_stop_is_queued_before_any_trigger_for_the_new_set() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = rig(256);
    let before = drain(&mut r.commands);
    assert!(!before.is_empty());
    let dir = tmp.path().join("kit");
    r.state.new_set(dir.to_str().unwrap()).unwrap();
    let queued = drain(&mut r.commands);
    assert!(
        matches!(queued.first(), Some(Command::StopAll)),
        "{queued:?}"
    );
}

#[test]
fn a_full_command_queue_refuses_the_switch_and_keeps_the_old_set() {
    let tmp = tempfile::tempdir().unwrap();
    // Startup leaves one slot free; a 14-command transition fits only after draining.
    let mut r = rig(14);
    r.state.rename_bank(0, "Mine".into());
    let dir = tmp.path().join("new");
    let error = r.state.new_set(dir.to_str().unwrap()).unwrap_err();
    assert!(error.contains("busy"), "{error}");
    assert_eq!(r.state.get_current_set().banks[0].name, "Mine");
    assert!(
        !dir.join("set.json").exists(),
        "the folder was handed back so a retry works"
    );

    drain(&mut r.commands);
    r.state.new_set(dir.to_str().unwrap()).unwrap();
    assert_eq!(r.state.get_current_set().banks[0].name, "Bank 1");
}

// ---- finding 3: never replace an existing kit -------------------------------------------

fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(&p).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

#[test]
fn creating_a_set_never_touches_an_existing_kit_or_the_active_set() {
    for existing in ["set.json", "set.json.bak", "set.json.broken"] {
        let tmp = tempfile::tempdir().unwrap();
        let r = rig(256);
        r.state.rename_bank(0, "Active".into());
        let dir = tmp.path().join("kit");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(existing), b"precious bytes, maybe not even json").unwrap();
        let before = files(&dir);

        let error = r.state.new_set(dir.to_str().unwrap()).unwrap_err();
        assert!(error.contains(existing), "{error}");
        assert_eq!(files(&dir), before, "{existing}: folder untouched");
        assert_eq!(r.state.get_current_set().banks[0].name, "Active");
        assert!(!r.state.history_state().can_redo);
    }
}

#[test]
fn creating_a_set_in_a_valid_kit_folder_keeps_its_backup_chain() {
    let tmp = tempfile::tempdir().unwrap();
    let r = rig(256);
    let dir = tmp.path().join("kit");
    r.state.new_set(dir.to_str().unwrap()).unwrap();
    r.state.rename_bank(0, "v1".into());
    r.state.save_set().unwrap();
    r.state.rename_bank(0, "v2".into());
    r.state.save_set().unwrap();
    let before = files(&dir);
    assert!(r.state.new_set(dir.to_str().unwrap()).is_err());
    assert_eq!(files(&dir), before);
    assert_eq!(r.state.get_current_set().banks[0].name, "v2");
}

#[test]
fn creating_a_set_in_a_fresh_folder_still_works() {
    let tmp = tempfile::tempdir().unwrap();
    let r = rig(256);
    let dir = tmp.path().join("a").join("b");
    let set = r.state.new_set(dir.to_str().unwrap()).unwrap().set;
    assert_eq!(set.banks.len(), 8);
    assert!(dir.join("set.json").exists());
}
