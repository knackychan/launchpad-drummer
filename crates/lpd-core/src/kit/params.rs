use super::{Group, Pad, PadMode};
use crate::audio::{fader_gain, GroupParams, PlayParams, Sample, GROUPS};

pub fn play_params(pad: &Pad, sample: &Sample) -> PlayParams {
    let frames = sample.frames();
    let to_frame = |ms: f32| ((ms.max(0.0) / 1000.0) * sample.rate as f32) as usize;
    let start = to_frame(pad.trim_start_ms).min(frames);
    let end = pad
        .trim_end_ms
        .map(to_frame)
        .unwrap_or(frames)
        .clamp(start, frames);
    PlayParams {
        gain: fader_gain(pad.volume_db),
        pitch_semitones: pad.pitch_semitones,
        start_frame: start,
        end_frame: end,
        choke_group: pad.choke_group,
        mix_group: pad.mix_group.min(GROUPS as u8 - 1),
        hold: pad.mode == PadMode::Hold,
    }
}

pub fn group_params(g: &Group) -> GroupParams {
    GroupParams {
        gain: fader_gain(g.volume_db),
        pan: g.pan,
        pitch_semitones: g.pitch_semitones,
        lowpass_hz: g.lowpass_hz,
        mute: g.mute,
        solo: g.solo,
    }
}
