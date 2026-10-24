//! The factory patches (spec 8.3). Recipes are starting points; Task 19 tunes them by ear.

use super::params::*;
use super::patch::{Category, Patch};

pub const FACTORY_IDS: [&str; 16] = [
    "piano",
    "ePiano",
    "organ",
    "synthPad",
    "synthLead",
    "synthBass",
    "moogBass",
    "acidBass",
    "wobbleBass",
    "supersaw",
    "syncLead",
    "pluck",
    "brass",
    "strings",
    "vowelPad",
    "fmBell",
];

const SINE: f32 = 0.0;
const TRIANGLE: f32 = 1.0 / 3.0;
const SAW: f32 = 2.0 / 3.0;
const SQUARE: f32 = 1.0;

fn route(source: Source, dest: Dest, amount: f32) -> Route {
    Route {
        source,
        dest: Some(dest),
        amount,
        aux: None,
        bypass: false,
    }
}

fn vibrato(p: &mut SynthParams, lfo: usize) {
    p.lfo[lfo] = LfoParams {
        shape: LfoShape::Sine,
        rate: 5.5,
        mode: LfoMode::Trig,
        ..LfoParams::default()
    };
    let source = if lfo == 0 { Source::Lfo1 } else { Source::Lfo2 };
    p.routes.push(Route {
        aux: Some(Source::Wheel),
        ..route(source, Dest::OscPitch, 0.021)
    });
}

fn env(attack: f32, decay: f32, sustain: f32, release: f32) -> EnvParams {
    EnvParams {
        attack,
        decay,
        sustain,
        release,
    }
}

fn osc(table: TableId, pos: f32) -> OscParams {
    OscParams {
        table,
        pos,
        ..OscParams::default()
    }
}

fn second(table: TableId, pos: f32) -> OscParams {
    OscParams {
        on: true,
        ..osc(table, pos)
    }
}

fn filter(kind: FilterType, cutoff: f32) -> FilterParams {
    FilterParams {
        kind,
        cutoff,
        ..FilterParams::default()
    }
}

pub fn factory(id: &str) -> Option<Patch> {
    use TableId::{Basic, Harmonics, Organ, Pwm, Vowels};
    let mut p = SynthParams::default();
    let mut lfo_for_vibrato = 0;
    let (name, category) = match id {
        "piano" => {
            p.osc_a = osc(Harmonics, 0.15);
            p.osc_b = OscParams {
                oct: 1,
                level: 0.3,
                ..second(Basic, TRIANGLE)
            };
            p.filter = FilterParams {
                key_track: 0.5,
                env_amt: 0.3,
                ..filter(FilterType::Lp12, 3_000.0)
            };
            p.env[0] = env(0.002, 1.2, 0.0, 0.25);
            p.env[1] = env(0.001, 0.8, 0.0, 0.2);
            p.routes.push(route(Source::Vel, Dest::Cutoff, 0.2));
            p.fx.reverb = ReverbFx {
                on: true,
                mix: 0.15,
                ..ReverbFx::default()
            };
            ("PIANO", Category::Piano)
        }
        "ePiano" => {
            p.osc_a = OscParams {
                warp: Warp::Fm,
                warp_amt: 0.1,
                ..osc(Basic, SINE)
            };
            p.osc_b = OscParams {
                oct: 1,
                level: 0.0,
                ..second(Basic, SINE)
            };
            p.env[2] = env(0.001, 0.4, 0.0, 0.15);
            p.routes.push(route(Source::Env3, Dest::OscAWarp, 0.3));
            p.env[0] = env(0.002, 1.5, 0.3, 0.4);
            p.fx.chorus = ChorusFx {
                on: true,
                mix: 0.3,
                ..ChorusFx::default()
            };
            ("E.PIANO", Category::Keys)
        }
        "organ" => {
            p.osc_a = osc(Organ, 0.5);
            p.filter.on = false;
            p.env[0] = env(0.005, 0.0, 1.0, 0.005);
            p.fx.chorus = ChorusFx {
                on: true,
                rate: 5.5,
                depth: 0.7,
                mix: 0.4,
                ..ChorusFx::default()
            };
            ("ORGAN", Category::Organ)
        }
        "synthPad" => {
            p.osc_a = OscParams {
                unison: 3,
                detune: 0.26,
                width: 0.8,
                ..osc(Basic, SAW)
            };
            p.filter = filter(FilterType::Lp12, 2_500.0);
            p.env[0] = env(0.3, 0.0, 1.0, 1.2);
            p.lfo[1] = LfoParams {
                shape: LfoShape::Triangle,
                rate: 0.3,
                mode: LfoMode::Free,
                ..LfoParams::default()
            };
            p.routes.push(route(Source::Lfo2, Dest::Cutoff, 0.1));
            p.fx.chorus = ChorusFx {
                on: true,
                mix: 0.3,
                ..ChorusFx::default()
            };
            p.fx.reverb = ReverbFx {
                on: true,
                mix: 0.35,
                decay: 4.0,
                ..ReverbFx::default()
            };
            ("SYNTH PAD", Category::Pad)
        }
        "synthLead" => {
            p.osc_a = osc(Basic, SQUARE);
            p.osc_b = OscParams {
                level: 0.55,
                ..second(Basic, SAW)
            };
            p.filter = FilterParams {
                reso: 0.2,
                ..filter(FilterType::Lp24, 4_000.0)
            };
            p.env[0] = env(0.005, 0.2, 0.7, 0.15);
            p.voice = VoiceParams {
                mode: VoiceMode::Legato,
                glide: 0.04,
                ..VoiceParams::default()
            };
            p.fx.delay = DelayFx {
                on: true,
                sync: true,
                sync_div: SyncDiv::D1_8D,
                mix: 0.2,
                ..DelayFx::default()
            };
            ("SYNTH LEAD", Category::Lead)
        }
        "synthBass" => {
            p.osc_a = OscParams {
                oct: -1,
                ..osc(Basic, SAW)
            };
            p.filter = filter(FilterType::Lp24, 800.0);
            p.env[0] = env(0.003, 0.3, 0.6, 0.08);
            ("SYNTH BASS", Category::Bass)
        }
        "moogBass" => {
            p.osc_a = osc(Basic, SAW);
            p.osc_b = OscParams {
                oct: -1,
                fine: -7.0,
                ..second(Basic, SQUARE)
            };
            p.sub = SubParams {
                on: true,
                level: 0.5,
                ..SubParams::default()
            };
            p.filter = FilterParams {
                reso: 0.3,
                drive: 0.4,
                env_amt: 0.45,
                ..filter(FilterType::Lp24, 300.0)
            };
            p.env[1] = env(0.0, 0.35, 0.0, 0.12);
            p.voice = VoiceParams {
                mode: VoiceMode::Mono,
                glide: 0.06,
                glide_mode: GlideMode::Legato,
                drift: 0.1,
                ..VoiceParams::default()
            };
            ("MOOG BASS", Category::Bass)
        }
        "acidBass" => {
            p.osc_a = osc(Basic, SAW);
            p.filter = FilterParams {
                reso: 0.75,
                drive: 0.3,
                env_amt: 0.6,
                ..filter(FilterType::Lp24, 400.0)
            };
            p.env[1] = env(0.001, 0.2, 0.0, 0.1);
            p.routes.push(route(Source::Vel, Dest::Cutoff, 0.25));
            p.voice = VoiceParams {
                mode: VoiceMode::Mono,
                glide: 0.08,
                glide_mode: GlideMode::Legato,
                ..VoiceParams::default()
            };
            p.fx.drive = DriveFx {
                on: true,
                kind: DriveType::Tube,
                amount: 0.3,
                ..DriveFx::default()
            };
            ("ACID BASS", Category::Bass)
        }
        "wobbleBass" => {
            p.osc_a = OscParams {
                unison: 3,
                detune: 0.2,
                ..osc(Basic, SAW)
            };
            p.osc_b = OscParams {
                oct: -1,
                ..second(Basic, SQUARE)
            };
            p.filter = FilterParams {
                reso: 0.4,
                ..filter(FilterType::Lp24, 250.0)
            };
            p.lfo[0] = LfoParams {
                sync: true,
                sync_div: SyncDiv::D1_8,
                ..LfoParams::default()
            };
            p.routes.push(route(Source::Lfo1, Dest::Cutoff, 0.55));
            p.fx.drive = DriveFx {
                on: true,
                kind: DriveType::Fold,
                amount: 0.25,
                ..DriveFx::default()
            };
            lfo_for_vibrato = 1;
            ("WOBBLE BASS", Category::Bass)
        }
        "supersaw" => {
            p.osc_a = OscParams {
                unison: 7,
                detune: 0.45,
                width: 1.0,
                blend: 0.8,
                ..osc(Basic, SAW)
            };
            p.osc_b = OscParams {
                oct: 1,
                unison: 7,
                detune: 0.3,
                level: 0.5,
                ..second(Basic, SAW)
            };
            p.filter = filter(FilterType::Lp12, 6_000.0);
            p.env[0] = env(0.005, 0.0, 1.0, 0.3);
            p.fx.delay = DelayFx {
                on: true,
                sync: true,
                sync_div: SyncDiv::D1_4,
                mix: 0.18,
                ..DelayFx::default()
            };
            p.fx.reverb = ReverbFx {
                on: true,
                mix: 0.25,
                ..ReverbFx::default()
            };
            ("SUPERSAW", Category::Lead)
        }
        "syncLead" => {
            p.osc_a = OscParams {
                warp: Warp::Sync,
                warp_amt: 0.3,
                ..osc(Basic, SAW)
            };
            p.routes.push(route(Source::Env2, Dest::OscAWarp, 0.4));
            p.osc_b = OscParams {
                oct: -1,
                level: 0.4,
                ..second(Basic, SQUARE)
            };
            p.filter = filter(FilterType::Lp24, 5_000.0);
            p.voice = VoiceParams {
                mode: VoiceMode::Mono,
                glide: 0.03,
                ..VoiceParams::default()
            };
            p.fx.delay = DelayFx {
                on: true,
                mix: 0.15,
                ..DelayFx::default()
            };
            ("SYNC LEAD", Category::Lead)
        }
        "pluck" => {
            p.osc_a = OscParams {
                unison: 3,
                detune: 0.15,
                ..osc(Basic, SAW)
            };
            p.osc_b = OscParams {
                level: 0.4,
                ..second(Harmonics, 0.6)
            };
            p.filter = FilterParams {
                env_amt: 0.55,
                ..filter(FilterType::Lp24, 600.0)
            };
            p.env[0] = env(0.001, 0.45, 0.0, 0.3);
            p.env[1] = env(0.001, 0.25, 0.0, 0.2);
            p.fx.reverb = ReverbFx {
                on: true,
                mix: 0.2,
                ..ReverbFx::default()
            };
            ("PLUCK", Category::Pluck)
        }
        "brass" => {
            p.osc_a = osc(Basic, SAW);
            p.osc_b = OscParams {
                fine: 8.0,
                ..second(Basic, SAW)
            };
            p.filter = FilterParams {
                env_amt: 0.35,
                ..filter(FilterType::Lp24, 900.0)
            };
            p.env[1] = env(0.08, 0.4, 0.4, 0.2);
            p.env[0] = env(0.04, 0.0, 0.9, 0.2);
            p.fx.reverb = ReverbFx {
                on: true,
                mix: 0.15,
                ..ReverbFx::default()
            };
            ("BRASS", Category::Lead)
        }
        "strings" => {
            p.osc_a = OscParams {
                unison: 5,
                detune: 0.22,
                width: 0.9,
                ..osc(Basic, SAW)
            };
            p.osc_b = OscParams {
                unison: 3,
                ..second(Pwm, 0.3)
            };
            p.filter = filter(FilterType::Lp12, 3_500.0);
            p.env[0] = env(0.45, 0.0, 1.0, 0.9);
            p.fx.chorus = ChorusFx {
                on: true,
                mix: 0.4,
                ..ChorusFx::default()
            };
            p.fx.reverb = ReverbFx {
                on: true,
                mix: 0.35,
                ..ReverbFx::default()
            };
            ("STRINGS", Category::Pad)
        }
        "vowelPad" => {
            p.osc_a = OscParams {
                unison: 3,
                ..osc(Vowels, 0.5)
            };
            p.lfo[1] = LfoParams {
                shape: LfoShape::Triangle,
                rate: 0.15,
                mode: LfoMode::Free,
                ..LfoParams::default()
            };
            p.routes.push(route(Source::Lfo2, Dest::OscAPos, 0.5));
            p.filter = filter(FilterType::Lp12, 4_000.0);
            p.env[0] = env(0.6, 0.0, 1.0, 1.5);
            p.fx.reverb = ReverbFx {
                on: true,
                mix: 0.45,
                ..ReverbFx::default()
            };
            ("VOWEL PAD", Category::Pad)
        }
        "fmBell" => {
            p.osc_a = OscParams {
                warp: Warp::Fm,
                warp_amt: 0.15,
                ..osc(Basic, SINE)
            };
            p.osc_b = OscParams {
                oct: 1,
                semi: 9,
                fine: 69.0,
                level: 0.0,
                ..second(Basic, SINE)
            };
            p.env[2] = env(0.001, 1.5, 0.0, 0.5);
            p.routes.push(route(Source::Env3, Dest::OscAWarp, 0.45));
            p.env[0] = env(0.001, 2.5, 0.0, 1.5);
            p.fx.reverb = ReverbFx {
                on: true,
                mix: 0.3,
                ..ReverbFx::default()
            };
            ("FM BELL", Category::Keys)
        }
        _ => return None,
    };
    vibrato(&mut p, lfo_for_vibrato);
    Some(Patch::new(name, category, p))
}

pub fn all() -> Vec<Patch> {
    FACTORY_IDS.iter().filter_map(|id| factory(id)).collect()
}
