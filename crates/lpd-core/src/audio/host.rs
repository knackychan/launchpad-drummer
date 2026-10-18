//! The body of the audio callback, shared by every audio backend.

use super::{Command, Engine, MAX_BLOCK};
use rtrb::Consumer;

/// Stereo scratch buffer for `process`. Allocate once, outside the callback.
pub fn scratch() -> Vec<f32> {
    vec![0.0; MAX_BLOCK * 2]
}

/// Apply queued commands, render, and write `out` (interleaved, `channels` per frame).
/// Mono devices get (L + R) / 2; channels beyond 2 are silent. Never allocates.
pub fn process(
    engine: &mut Engine,
    commands: &mut Consumer<Command>,
    scratch: &mut [f32],
    out: &mut [f32],
    channels: usize,
) {
    while let Ok(cmd) = commands.pop() {
        engine.handle(cmd);
    }
    if channels == 0 {
        return;
    }
    let frames_per_chunk = scratch.len() / 2;
    for chunk in out.chunks_mut(frames_per_chunk * channels) {
        let frames = chunk.len() / channels;
        let stereo = &mut scratch[..frames * 2];
        engine.render(stereo);
        for (f, frame) in chunk.chunks_mut(channels).enumerate() {
            let (l, r) = (stereo[2 * f], stereo[2 * f + 1]);
            if channels == 1 {
                frame[0] = 0.5 * (l + r);
            } else {
                frame[0] = l;
                frame[1] = r;
                frame[2..].fill(0.0);
            }
        }
    }
}
