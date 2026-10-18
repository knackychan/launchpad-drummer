/// Decoded audio: interleaved stereo f32 at `rate` Hz.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub rate: u32,
    pub data: Vec<f32>,
}

impl Sample {
    pub fn frames(&self) -> usize {
        self.data.len() / 2
    }

    /// Left/right of frame `i`; silence outside the sample.
    #[inline]
    pub fn frame(&self, i: isize) -> (f32, f32) {
        if i < 0 || i as usize >= self.frames() {
            return (0.0, 0.0);
        }
        let j = i as usize * 2;
        (self.data[j], self.data[j + 1])
    }
}
