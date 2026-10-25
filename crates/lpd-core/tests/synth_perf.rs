use lpd_core::instrument::Instrument;
use lpd_core::synth::params::*;
use lpd_core::synth::Synth;

/// Worst case (spec 11): 16 notes, both oscillators at unison 7, LP 24 with drive, all four FX.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_synth_meets_its_cpu_budget() {
    let mut p = SynthParams::default();
    p.osc_a.unison = 7;
    p.osc_b = OscParams {
        on: true,
        unison: 7,
        ..OscParams::default()
    };
    p.sub.on = true;
    p.noise.on = true;
    p.filter.drive = 0.5;
    p.fx.drive.on = true;
    p.fx.chorus.on = true;
    p.fx.delay.on = true;
    p.fx.reverb.on = true;
    let mut s = Synth::new(48_000);
    s.load_params(&p);
    let (mut l, mut r) = ([0.0f32; 128], [0.0f32; 128]);
    s.render_add(&mut l, &mut r);
    s.render_add(&mut l, &mut r);
    for n in 0..16 {
        s.note_on(48 + n * 2, 100);
    }
    let blocks = 48_000 * 10 / 128;
    let t0 = std::time::Instant::now();
    for _ in 0..blocks {
        s.render_add(&mut l, &mut r);
    }
    let elapsed = t0.elapsed().as_secs_f32();
    eprintln!("10 s of worst-case synth audio rendered in {elapsed:.3} s");
    assert!(elapsed < 2.5, "{elapsed} s is over 25 % of real time");
}
