use std::f32::consts::{FRAC_1_SQRT_2, PI};

/// RBJ-cookbook 2nd-order low-pass (Q = 1/√2), transposed direct form II.
#[derive(Debug, Clone, Copy)]
pub struct LowPass {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
    bypass: bool,
}

impl LowPass {
    pub fn new() -> LowPass {
        LowPass {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
            bypass: true,
        }
    }

    /// At or above 20 kHz (or 0.45 × sample rate) the filter is bypassed.
    pub fn set_cutoff(&mut self, hz: f32, sample_rate: f32) {
        if hz >= 20_000.0 || hz >= 0.45 * sample_rate {
            self.bypass = true;
            return;
        }
        let w0 = 2.0 * PI * hz.max(20.0) / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * FRAC_1_SQRT_2);
        let a0 = 1.0 + alpha;
        self.b0 = (1.0 - cos) / 2.0 / a0;
        self.b1 = (1.0 - cos) / a0;
        self.b2 = self.b0;
        self.a1 = -2.0 * cos / a0;
        self.a2 = (1.0 - alpha) / a0;
        if self.bypass {
            self.z1 = 0.0;
            self.z2 = 0.0;
        }
        self.bypass = false;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        if self.bypass {
            return x;
        }
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

impl Default for LowPass {
    fn default() -> Self {
        LowPass::new()
    }
}
