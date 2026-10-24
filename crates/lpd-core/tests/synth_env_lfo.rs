use lpd_core::synth::env::{Env, Stage, TICK};
use lpd_core::synth::lfo::{rate_hz, shape_value, Lfo};
use lpd_core::synth::params::{EnvParams, LfoMode, LfoParams, LfoShape, Polarity, SyncDiv};

const DT: f32 = TICK as f32 / 48_000.0;

/// Seconds until `done` holds, ticking `env` with `p`.
fn time_until(env: &mut Env, p: &EnvParams, done: impl Fn(&Env) -> bool) -> f32 {
    let mut t = 0.0;
    while !done(env) {
        env.tick(p, DT);
        t += DT;
        assert!(t < 60.0, "never reached");
    }
    t
}

#[test]
fn the_attack_reaches_full_level_in_its_time() {
    let p = EnvParams {
        attack: 0.1,
        decay: 0.5,
        sustain: 0.5,
        release: 0.2,
    };
    let mut e = Env::default();
    e.gate_on();
    let t = time_until(&mut e, &p, |e| e.stage != Stage::Attack);
    assert!((t - 0.1).abs() <= DT * 1.01, "{t}");
    assert_eq!(e.level, 1.0);
}

#[test]
fn the_decay_reaches_one_percent_of_the_way_in_its_time() {
    let p = EnvParams {
        attack: 0.0,
        decay: 0.5,
        sustain: 0.3,
        release: 0.2,
    };
    let mut e = Env::default();
    e.gate_on();
    time_until(&mut e, &p, |e| e.stage == Stage::Decay);
    let t = time_until(&mut e, &p, |e| e.level - 0.3 <= 0.007 + 1e-6);
    assert!((t - 0.5).abs() <= DT * 1.01, "{t}");
}

#[test]
fn the_release_ends_the_envelope() {
    let p = EnvParams {
        attack: 0.0,
        decay: 0.0,
        sustain: 1.0,
        release: 0.2,
    };
    let mut e = Env::default();
    e.gate_on();
    time_until(&mut e, &p, |e| e.stage == Stage::Sustain);
    e.gate_off();
    // 1e-4 is reached after log(1e-4)/log(0.01) = 2 release times.
    let t = time_until(&mut e, &p, Env::is_idle);
    assert!((t - 0.4).abs() <= DT * 1.01, "{t}");
    assert_eq!(e.level, 0.0);
}

#[test]
fn a_retrigger_starts_from_the_current_level() {
    let p = EnvParams {
        attack: 0.05,
        decay: 0.0,
        sustain: 1.0,
        release: 1.0,
    };
    let mut e = Env::default();
    e.gate_on();
    time_until(&mut e, &p, |e| e.stage == Stage::Sustain);
    e.gate_off();
    time_until(&mut e, &p, |e| e.level < 0.4);
    let before = e.level;
    e.gate_on();
    assert!(e.tick(&p, DT) > before);
}

#[test]
fn a_kill_releases_in_five_ms() {
    let p = EnvParams {
        attack: 0.0,
        decay: 0.0,
        sustain: 1.0,
        release: 10.0,
    };
    let mut e = Env::default();
    e.gate_on();
    time_until(&mut e, &p, |e| e.stage == Stage::Sustain);
    e.kill();
    let t = time_until(&mut e, &p, Env::is_idle);
    assert!(t <= 0.0105, "{t}");
}

#[test]
fn the_rate_sets_the_period() {
    let p = LfoParams::default();
    let mut lfo = Lfo::new(1);
    let mut wraps = 0;
    let mut last = lfo.phase;
    for _ in 0..(10.0 / DT) as usize {
        lfo.tick(&p, 2.0, DT);
        if lfo.phase < last {
            wraps += 1;
        }
        last = lfo.phase;
    }
    assert!((19..=21).contains(&wraps), "{wraps}");
}

#[test]
fn sync_follows_the_tempo() {
    let p = LfoParams {
        sync: true,
        sync_div: SyncDiv::D1_4,
        ..LfoParams::default()
    };
    assert!((rate_hz(&p, 5.5, 120.0) - 2.0).abs() < 1e-5);
    let dotted = LfoParams {
        sync: true,
        sync_div: SyncDiv::D1_8D,
        ..LfoParams::default()
    };
    assert!((rate_hz(&dotted, 5.5, 100.0) - 2.222_22).abs() < 1e-4);
    assert_eq!(rate_hz(&LfoParams::default(), 5.5, 120.0), 5.5);
}

#[test]
fn trig_restarts_at_the_phase() {
    let p = LfoParams {
        phase: 0.25,
        ..LfoParams::default()
    };
    let mut lfo = Lfo::new(1);
    lfo.tick(&p, 3.0, 0.1);
    lfo.restart(&p);
    assert_eq!(lfo.phase, 0.25);
}

#[test]
fn one_shot_holds_after_one_cycle() {
    let p = LfoParams {
        mode: LfoMode::OneShot,
        shape: LfoShape::SawUp,
        ..LfoParams::default()
    };
    let mut lfo = Lfo::new(1);
    lfo.restart(&p);
    for _ in 0..(0.2 / DT) as usize {
        lfo.tick(&p, 10.0, DT);
    }
    let held = lfo.value;
    assert!(held > 0.99, "{held}");
    for _ in 0..100 {
        assert_eq!(lfo.tick(&p, 10.0, DT), held);
    }
}

#[test]
fn sample_and_hold_steps_once_per_cycle() {
    let p = LfoParams {
        shape: LfoShape::SampleHold,
        mode: LfoMode::Free,
        ..LfoParams::default()
    };
    let mut lfo = Lfo::new(9);
    let mut changes = 0;
    let mut last = lfo.tick(&p, 4.0, DT);
    for _ in 0..(1.0 / DT) as usize {
        let v = lfo.tick(&p, 4.0, DT);
        if v != last {
            changes += 1;
        }
        last = v;
    }
    assert!((3..=5).contains(&changes), "{changes}");
}

#[test]
fn delay_fades_the_lfo_in() {
    let p = LfoParams {
        delay: 1.0,
        shape: LfoShape::Square,
        ..LfoParams::default()
    };
    let mut lfo = Lfo::new(1);
    lfo.restart(&p);
    for _ in 0..(0.5 / DT) as usize {
        lfo.tick(&p, 2.0, DT);
    }
    assert!(lfo.value.abs() <= 0.51, "{}", lfo.value);
    for _ in 0..(0.7 / DT) as usize {
        lfo.tick(&p, 2.0, DT);
    }
    assert!((lfo.value.abs() - 1.0).abs() < 1e-6);
}

#[test]
fn unipolar_stays_positive_and_shapes_are_in_range() {
    let p = LfoParams {
        polarity: Polarity::Uni,
        ..LfoParams::default()
    };
    let mut lfo = Lfo::new(1);
    for _ in 0..3000 {
        let v = lfo.tick(&p, 7.0, DT);
        assert!((0.0..=1.0).contains(&v));
    }
    for shape in [
        LfoShape::Sine,
        LfoShape::Triangle,
        LfoShape::SawUp,
        LfoShape::SawDown,
        LfoShape::Square,
    ] {
        for i in 0..100 {
            assert!(shape_value(shape, i as f32 / 100.0).abs() <= 1.0);
        }
    }
    assert_eq!(shape_value(LfoShape::Triangle, 0.25), 1.0);
}
