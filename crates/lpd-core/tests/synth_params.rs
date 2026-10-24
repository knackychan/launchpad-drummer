use lpd_core::synth::params::*;
use std::collections::HashSet;

fn float(id: ParamId) -> (f32, f32, Taper) {
    match id.meta().kind {
        Kind::Float {
            min, max, taper, ..
        } => (min, max, taper),
        other => panic!("{id:?} is not a float: {other:?}"),
    }
}

#[test]
fn every_param_id_round_trips_through_its_string() {
    assert_eq!(ParamId::ALL.len(), 109);
    let mut seen = HashSet::new();
    for &id in ParamId::ALL {
        let meta = id.meta();
        assert!(seen.insert(meta.id), "duplicate id {}", meta.id);
        assert_eq!(ParamId::from_id(meta.id), Some(id));
    }
    assert_eq!(ParamId::from_id("filter.nope"), None);
}

#[test]
fn defaults_match_the_spec() {
    let p = SynthParams::default();
    assert!(p.osc_a.on && !p.osc_b.on);
    assert_eq!(
        (p.osc_a.table, p.osc_a.pos, p.osc_a.unison),
        (TableId::Basic, 0.667, 1)
    );
    assert_eq!(
        (p.osc_a.detune, p.osc_a.blend, p.osc_a.width),
        (0.25, 0.75, 0.5)
    );
    assert_eq!(
        (p.osc_a.phase_rand, p.osc_a.level, p.osc_a.pan),
        (1.0, 0.75, 0.0)
    );
    assert!(!p.sub.on && p.sub.oct == -1 && p.sub.level == 0.6);
    assert!(!p.noise.on && p.noise.level == 0.3);
    assert_eq!(
        (p.filter.kind, p.filter.cutoff, p.filter.reso),
        (FilterType::Lp24, 12_000.0, 0.0)
    );
    assert_eq!((p.amp.level, p.amp.vel_sens), (0.8, 0.7));
    assert_eq!(p.env[0].sustain, 1.0);
    assert_eq!((p.env[1].sustain, p.env[2].sustain), (0.0, 0.0));
    assert_eq!(
        (p.env[0].attack, p.env[0].decay, p.env[0].release),
        (0.002, 0.5, 0.15)
    );
    assert_eq!(
        (p.lfo[0].shape, p.lfo[0].rate, p.lfo[0].mode),
        (LfoShape::Sine, 5.5, LfoMode::Trig)
    );
    assert_eq!(
        (p.lfo[1].shape, p.lfo[1].rate, p.lfo[1].mode),
        (LfoShape::Triangle, 0.5, LfoMode::Free)
    );
    assert_eq!(p.lfo[0].sync_div, SyncDiv::D1_4);
    assert_eq!(
        (p.voice.mode, p.voice.bend_range, p.voice.volume_db),
        (VoiceMode::Poly, 2, 0.0)
    );
    assert!(!p.fx.drive.on && !p.fx.chorus.on && !p.fx.delay.on && !p.fx.reverb.on);
    assert_eq!(p.fx.delay.sync_div, SyncDiv::D1_8D);
    assert!(p.routes.is_empty());
}

#[test]
fn tapers_map_known_points() {
    let cutoff = ParamId::FilterCutoff.meta().kind;
    assert!((cutoff.from_norm(0.5) - 632.456).abs() < 0.01);
    assert!((cutoff.to_norm(12_000.0) - 0.926_05).abs() < 1e-4);
    assert!((ParamId::Env1Attack.meta().kind.from_norm(0.5) - 1.25).abs() < 1e-6);
    assert!((ParamId::Lfo1Rate.meta().kind.from_norm(0.5) - 0.707_107).abs() < 1e-5);
    assert!((ParamId::FxDelayTime.meta().kind.from_norm(0.5) - 0.044_721_4).abs() < 1e-6);
    assert!((ParamId::FilterEnvAmt.meta().kind.from_norm(0.5)).abs() < 1e-6);
    for &id in ParamId::ALL {
        if let Kind::Float { min, taper, .. } = id.meta().kind {
            if taper == Taper::Cubic {
                assert_eq!(min, 0.0, "{id:?}: cubic tapers start at 0");
            }
            for n in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let k = id.meta().kind;
                assert!(
                    (k.to_norm(k.from_norm(n)) - n).abs() < 1e-4,
                    "{id:?} at {n}"
                );
            }
        }
    }
    let _ = float(ParamId::FilterCutoff);
}

#[test]
fn set_clamps_and_quantizes() {
    let mut p = SynthParams::default();
    ParamId::OscAOct.set(&mut p, 5.0);
    assert_eq!(ParamId::OscAOct.get(&p), 3.0);
    ParamId::OscAUnison.set(&mut p, 0.0);
    assert_eq!(p.osc_a.unison, 1);
    ParamId::OscAUnison.set(&mut p, 3.6);
    assert_eq!(p.osc_a.unison, 4);
    ParamId::FilterType.set(&mut p, 9.0);
    assert_eq!(p.filter.kind, FilterType::Notch);
    ParamId::OscBOn.set(&mut p, 0.7);
    assert!(p.osc_b.on);
    ParamId::FilterCutoff.set(&mut p, 5.0);
    assert_eq!(p.filter.cutoff, 20.0);
    ParamId::FilterCutoff.set(&mut p, f32::NAN);
    assert_eq!(p.filter.cutoff, 20.0);
    ParamId::FxDelaySyncDiv.set(&mut p, 14.0);
    assert_eq!(p.fx.delay.sync_div, SyncDiv::D1_2, "the delay stops at 1/2");
    ParamId::SubOct.set(&mut p, 0.0);
    assert_eq!(p.sub.oct, -1);
}

#[test]
fn route_table_appends_replaces_and_compacts() {
    let r = |amount: f32| Route {
        source: Source::Lfo1,
        dest: Some(Dest::Cutoff),
        amount,
        aux: None,
        bypass: false,
    };
    let mut t = RouteTable::default();
    assert!(t.push(r(0.1)) && t.push(r(0.2)) && t.push(r(0.3)));
    assert_eq!(t.len(), 3);
    assert!(t.set(1, None));
    assert_eq!(t.len(), 2);
    assert_eq!(t.get(1).unwrap().amount, 0.3);
    assert!(!t.set(5, Some(r(0.5))), "no gaps");
    assert!(t.set(0, Some(r(2.0))));
    assert_eq!(t.get(0).unwrap().amount, 1.0, "amounts are clamped to ±1");
    while t.len() < ROUTES {
        assert!(t.push(r(0.0)));
    }
    assert!(!t.push(r(0.0)), "16 routes at most");
}

#[test]
fn params_serialize_in_the_patch_shape() {
    let p = SynthParams::default();
    let v = serde_json::to_value(p).unwrap();
    for key in [
        "oscA", "oscB", "sub", "noise", "filter", "amp", "env", "lfo", "voice", "routes", "fx",
    ] {
        assert!(v.get(key).is_some(), "missing {key}");
    }
    assert_eq!(v["filter"]["type"], "lp24");
    assert_eq!(v["env"].as_array().unwrap().len(), 3);
    assert_eq!(v["lfo"][0]["syncDiv"], "1/4");
    assert_eq!(v["routes"], serde_json::json!([]));
    assert_eq!(v["fx"]["delay"]["pingPong"], false);
    let back: SynthParams = serde_json::from_value(v).unwrap();
    assert_eq!(back, p);
}

#[test]
fn missing_fields_take_defaults() {
    let p: SynthParams = serde_json::from_str(r#"{"filter":{"cutoff":300}}"#).unwrap();
    assert_eq!(p.filter.cutoff, 300.0);
    assert_eq!(p.filter.reso, 0.0);
    assert_eq!(p.osc_a, OscParams::default());
    let routed: SynthParams = serde_json::from_str(
        r#"{"routes":[{"source":"lfo1","dest":"osc.pitch","amount":0.021,"aux":"wheel"}]}"#,
    )
    .unwrap();
    let route = routed.routes.get(0).unwrap();
    assert_eq!(
        (route.source, route.dest, route.aux, route.bypass),
        (
            Source::Lfo1,
            Some(Dest::OscPitch),
            Some(Source::Wheel),
            false
        )
    );
}

#[test]
fn destinations_and_sources_are_complete() {
    assert_eq!(Dest::ALL.len(), DESTS);
    assert_eq!(Source::ALL.len(), SOURCES);
    assert_eq!(Dest::Cutoff.param(), Some(ParamId::FilterCutoff));
    assert_eq!(Dest::OscBWarp.param(), Some(ParamId::OscBWarpAmt));
    assert_eq!(Dest::Lfo2Rate.param(), Some(ParamId::Lfo2Rate));
    for d in [Dest::OscAPitch, Dest::OscBPitch, Dest::OscPitch] {
        assert!(d.param().is_none() && d.is_pitch());
    }
    assert_eq!(Dest::IDS[0], "oscA.pos");
    assert_eq!(Dest::IDS[26], "lfo2.rate");
    assert_eq!(Source::Lfo2.color(), "#7f8cff");
}
