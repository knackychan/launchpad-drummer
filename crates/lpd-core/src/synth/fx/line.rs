//! A circular delay line with fractional reads (read before write: delay 1 = previous sample).

pub struct DelayLine {
    buf: Vec<f32>,
    w: usize,
    pending: usize,
}

impl DelayLine {
    pub fn new(len: usize) -> DelayLine {
        DelayLine {
            buf: vec![0.0; len.max(4)],
            w: 0,
            pending: 0,
        }
    }

    /// The input of `delay` samples ago (fractional; clamped to 1 … len − 2).
    #[inline]
    pub fn read(&self, delay: f32) -> f32 {
        let len = self.buf.len();
        let d = delay.clamp(1.0, (len - 2) as f32);
        let mut pos = self.w as f32 - d;
        if pos < 0.0 {
            pos += len as f32;
        }
        let i = (pos as usize).min(len - 1);
        let fr = pos - i as f32;
        let a = self.buf[i];
        let b = self.buf[if i + 1 == len { 0 } else { i + 1 }];
        a + (b - a) * fr
    }

    #[inline]
    pub fn write(&mut self, x: f32) {
        self.pending -= usize::from(self.buf[self.w].abs() >= super::QUIET_IN);
        self.pending += usize::from(x.abs() >= super::QUIET_IN);
        self.buf[self.w] = x;
        self.w += 1;
        if self.w == self.buf.len() {
            self.w = 0;
        }
    }

    pub fn clear(&mut self) {
        self.buf.fill(0.0);
        self.pending = 0;
    }

    pub(super) fn pending(&self) -> bool {
        self.pending > 0
    }
}
