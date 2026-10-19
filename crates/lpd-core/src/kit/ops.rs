use super::learn::{retarget_pad, swap_pad_bindings};
use super::{Bank, Pad, Set};
use serde::Serialize;
use std::path::Path;

pub const AUDIO_EXTENSIONS: [&str; 6] = ["wav", "aif", "aiff", "flac", "mp3", "ogg"];
pub const PRESETS: [[u8; 3]; 16] = [
    [63, 0, 0],
    [63, 20, 0],
    [63, 40, 0],
    [63, 63, 0],
    [30, 63, 0],
    [0, 63, 0],
    [0, 63, 25],
    [0, 63, 63],
    [0, 30, 63],
    [0, 0, 63],
    [25, 0, 63],
    [45, 0, 63],
    [63, 0, 63],
    [63, 10, 30],
    [63, 63, 63],
    [63, 45, 30],
];
pub fn is_audio_path(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        AUDIO_EXTENSIONS
            .iter()
            .any(|ext| e.eq_ignore_ascii_case(ext))
    })
}
pub fn row_default_color(y: u8) -> [u8; 3] {
    PRESETS[[0, 1, 3, 5, 7, 9, 11, 13][usize::from(y.min(7))]]
}
pub fn drop_positions(bank: &Bank, x: u8, y: u8, n: usize) -> Vec<(u8, u8)> {
    if x > 7 || y > 7 {
        return vec![];
    }
    (y * 8 + x..64)
        .map(|i| (i % 8, i / 8))
        .filter(|&(px, py)| (px, py) == (x, y) || bank.pad(px, py).is_none())
        .take(n)
        .collect()
}
pub fn assign_sample(bank: &mut Bank, x: u8, y: u8, sample: String) {
    if x > 7 || y > 7 {
        return;
    }
    if let Some(p) = bank.pads.iter_mut().find(|p| (p.x, p.y) == (x, y)) {
        p.sample = sample;
        p.trim_start_ms = 0.0;
        p.trim_end_ms = None;
    } else {
        bank.set_pad(Pad::new(x, y, sample, row_default_color(y)));
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PadMove {
    Move,
    Swap,
    Copy,
}
pub fn move_pad(
    set: &mut Set,
    from: (usize, u8, u8),
    to: (usize, u8, u8),
    copy: bool,
) -> Option<PadMove> {
    let (fb, fx, fy) = from;
    let (tb, tx, ty) = to;
    if from == to || fx > 7 || fy > 7 || tx > 7 || ty > 7 {
        return None;
    }
    let mut source = set.banks.get(fb)?.pad(fx, fy)?.clone();
    let target = set.banks.get(tb)?.pad(tx, ty).cloned();
    source.x = tx;
    source.y = ty;
    let result = if copy {
        PadMove::Copy
    } else if target.is_some() {
        PadMove::Swap
    } else {
        PadMove::Move
    };
    if !copy {
        set.banks[fb].pads.retain(|p| (p.x, p.y) != (fx, fy));
        if let Some(mut p) = target {
            p.x = fx;
            p.y = fy;
            set.banks[fb].set_pad(p);
        }
    }
    set.banks[tb].set_pad(source);
    let (f, t) = ((fb as u8, fx, fy), (tb as u8, tx, ty));
    match result {
        PadMove::Move => {
            // The target had no sample but may carry a binding: the moved pad's wins.
            retarget_pad(&mut set.learn, t, None);
            retarget_pad(&mut set.learn, f, Some(t));
            crate::seq::retarget_lanes(&mut set.pattern, f, t);
        }
        PadMove::Swap => {
            swap_pad_bindings(&mut set.learn, f, t);
            crate::seq::swap_lanes(&mut set.pattern, f, t);
        }
        PadMove::Copy => {}
    }
    Some(result)
}
pub fn clear_pad(set: &mut Set, bank: usize, x: u8, y: u8) -> bool {
    let Some(b) = set.banks.get_mut(bank) else {
        return false;
    };
    let before = b.pads.len();
    b.pads.retain(|p| (p.x, p.y) != (x, y));
    let cleared = before != b.pads.len();
    retarget_pad(&mut set.learn, (bank as u8, x, y), None);
    cleared
}
