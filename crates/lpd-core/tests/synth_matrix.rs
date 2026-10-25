use lpd_core::synth::matrix::*;
use lpd_core::synth::params::*;

fn route(source: Source, dest: Dest, amount: f32) -> Route {
    Route {
        source,
        dest: Some(dest),
        amount,
        aux: None,
        bypass: false,
    }
}

fn sources(pairs: &[(Source, f32)]) -> Sources {
    let mut s = [0.0; SOURCES];
    for &(src, v) in pairs {
        s[src as usize] = v;
    }
    s
}

#[test]
fn routes_multiply_amount_source_and_aux() {
    let mut p = SynthParams::default();
    p.routes.push(route(Source::Lfo1, Dest::Cutoff, 0.5));
    let mut out = [0.0; DESTS];
    accumulate(&p.routes, &p, &sources(&[(Source::Lfo1, 0.4)]), &mut out);
    assert!((out[Dest::Cutoff as usize] - 0.2).abs() < 1e-6);
    p.routes.set(
        0,
        Some(Route {
            aux: Some(Source::Wheel),
            ..route(Source::Lfo1, Dest::Cutoff, 0.5)
        }),
    );
    accumulate(
        &p.routes,
        &p,
        &sources(&[(Source::Lfo1, 0.4), (Source::Wheel, 0.5)]),
        &mut out,
    );
    assert!((out[Dest::Cutoff as usize] - 0.1).abs() < 1e-6);
    p.routes.push(route(Source::Env2, Dest::Cutoff, 0.25));
    accumulate(
        &p.routes,
        &p,
        &sources(&[
            (Source::Lfo1, 0.4),
            (Source::Wheel, 0.5),
            (Source::Env2, 1.0),
        ]),
        &mut out,
    );
    assert!(
        (out[Dest::Cutoff as usize] - 0.35).abs() < 1e-6,
        "routes add up"
    );
    p.routes.set(
        1,
        Some(Route {
            bypass: true,
            ..route(Source::Env2, Dest::Cutoff, 0.25)
        }),
    );
    p.routes.push(Route {
        dest: None,
        ..route(Source::Vel, Dest::Cutoff, 1.0)
    });
    accumulate(
        &p.routes,
        &p,
        &sources(&[
            (Source::Lfo1, 0.4),
            (Source::Wheel, 0.5),
            (Source::Env2, 1.0),
            (Source::Vel, 1.0),
        ]),
        &mut out,
    );
    assert!(
        (out[Dest::Cutoff as usize] - 0.1).abs() < 1e-6,
        "bypassed and empty rows do nothing"
    );
}

#[test]
fn live_values_clamp_to_the_knob_range() {
    assert_eq!(live_norm(0.9, 0.3), 1.0);
    assert_eq!(live_norm(0.1, -0.3), 0.0);
    assert!((live_norm(0.5, 0.25) - 0.75).abs() < 1e-6);
}

#[test]
fn cutoff_moves_in_log_space() {
    let kind = ParamId::FilterCutoff.meta().kind;
    let base = kind.to_norm(1_000.0);
    assert!((kind.from_norm(live_norm(base, 0.1)) - 1_995.26).abs() < 0.5);
}

#[test]
fn pitch_destinations_are_semitones() {
    assert_eq!(pitch_semis(0.5), 12.0);
    assert_eq!(pitch_semis(-1.0), -24.0);
}

#[test]
fn a_free_lfo_rate_only_takes_global_sources() {
    let mut p = SynthParams::default(); // LFO2 is FREE, LFO1 is TRIG
    p.routes.push(route(Source::Env1, Dest::Lfo2Rate, 0.5));
    p.routes.push(route(Source::Wheel, Dest::Lfo2Rate, 0.25));
    p.routes.push(route(Source::Lfo1, Dest::Lfo2Rate, 0.5));
    let src = sources(&[
        (Source::Env1, 1.0),
        (Source::Wheel, 1.0),
        (Source::Lfo1, 1.0),
    ]);
    let mut out = [0.0; DESTS];
    accumulate(&p.routes, &p, &src, &mut out);
    assert_eq!(
        out[Dest::Lfo2Rate as usize],
        0.0,
        "a FREE LFO's rate is not per voice"
    );
    assert!((accumulate_global(&p.routes, &p, &src, 1) - 0.25).abs() < 1e-6);
    p.lfo[0].mode = LfoMode::Free;
    assert!(
        (accumulate_global(&p.routes, &p, &src, 1) - 0.75).abs() < 1e-6,
        "a FREE LFO1 is global too"
    );
    assert!(is_global(Source::Bend, &p) && !is_global(Source::Vel, &p));
}

#[test]
fn sync_ignores_rate_routes() {
    let mut p = SynthParams::default();
    p.lfo[0].sync = true;
    p.routes.push(route(Source::Env1, Dest::Lfo1Rate, 0.5));
    let mut out = [0.0; DESTS];
    accumulate(&p.routes, &p, &sources(&[(Source::Env1, 1.0)]), &mut out);
    assert_eq!(out[Dest::Lfo1Rate as usize], 0.0);
}

#[test]
fn base_norms_follow_the_knobs() {
    let mut p = SynthParams::default();
    p.filter.cutoff = 1_000.0;
    p.osc_a.pan = -1.0;
    let b = base_norms(&p);
    assert!((b[Dest::Cutoff as usize] - 0.566_323).abs() < 1e-5);
    assert_eq!(b[Dest::OscAPan as usize], 0.0);
    assert_eq!(b[Dest::OscPitch as usize], 0.0);
    assert!((b[Dest::OscAPos as usize] - 0.667).abs() < 1e-6);
}
