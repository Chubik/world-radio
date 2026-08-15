use rustfft::{num_complex::Complex, FftPlanner};

const FFT_SIZE: usize = 1024;
const DECAY: f32 = 0.6;

pub struct Spectrum {
    planner: FftPlanner<f32>,
    prev: Vec<f32>,
    divisor: f32,
}

impl Spectrum {
    pub fn new() -> Spectrum {
        Spectrum {
            planner: FftPlanner::new(),
            prev: Vec::new(),
            divisor: 12.0,
        }
    }

    pub fn set_divisor(&mut self, d: f32) {
        self.divisor = d.max(0.1);
    }

    #[allow(clippy::needless_range_loop)]
    pub fn analyze(&mut self, samples: &[f32], bars: usize) -> Vec<f32> {
        if bars == 0 {
            return Vec::new();
        }
        if samples.len() < 2 {
            if self.prev.len() != bars {
                self.prev = vec![0.0; bars];
            }
            return self.prev.clone();
        }

        let n = FFT_SIZE.min(samples.len());
        let fft = self.planner.plan_fft_forward(n);
        let mut buf: Vec<Complex<f32>> = samples[..n]
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                let w = hann(i, n);
                Complex { re: s * w, im: 0.0 }
            })
            .collect();
        fft.process(&mut buf);

        let half = n / 2;
        let mut raw = vec![0.0_f32; bars];
        for bar in 0..bars {
            let (lo, hi) = log_bin_range(bar, bars, half);
            let mut sum = 0.0;
            for bin in lo..hi {
                sum += (buf[bin].re * buf[bin].re + buf[bin].im * buf[bin].im).sqrt();
            }
            let mag = sum / (hi - lo) as f32;
            raw[bar] = (1.0 + mag).ln() / self.divisor;
        }

        if self.prev.len() != bars {
            self.prev = vec![0.0; bars];
        }
        let mut out = vec![0.0_f32; bars];
        for i in 0..bars {
            let decayed = self.prev[i] * DECAY;
            out[i] = raw[i].max(decayed).clamp(0.0, 1.0);
        }
        self.prev = out.clone();
        out
    }
}

fn hann(i: usize, n: usize) -> f32 {
    let x = std::f32::consts::PI * i as f32 / (n - 1) as f32;
    x.sin().powi(2)
}

fn log_bin_range(bar: usize, bars: usize, half: usize) -> (usize, usize) {
    let min_bin = 1.0_f32;
    let max_bin = half as f32;
    let ratio = max_bin / min_bin;
    let edge = |k: usize| -> usize {
        let frac = k as f32 / bars as f32;
        (min_bin * ratio.powf(frac)).round() as usize
    };
    let lo = edge(bar).min(half - 1);
    let hi = edge(bar + 1).max(lo + 1).min(half);
    (lo, hi)
}

impl Default for Spectrum {
    fn default() -> Self {
        Spectrum::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn analyze_returns_requested_bar_count_in_unit_range() {
        let mut sp = Spectrum::new();
        let samples: Vec<f32> = (0..2048)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        let bars = sp.analyze(&samples, 16);
        assert_eq!(bars.len(), 16);
        assert!(bars.iter().all(|&b| (0.0..=1.0).contains(&b)));
    }

    #[test]
    fn silence_produces_near_zero_bars() {
        let mut sp = Spectrum::new();
        let bars = sp.analyze(&[0.0_f32; 2048], 8);
        assert!(bars.iter().all(|&b| b < 0.05));
    }

    #[test]
    fn empty_input_returns_zeroed_bars_without_panicking() {
        let mut sp = Spectrum::new();
        let bars = sp.analyze(&[], 12);
        assert_eq!(bars.len(), 12);
        assert!(bars.iter().all(|&b| b == 0.0));
    }

    #[test]
    fn single_sample_input_returns_finite_bars_without_nan() {
        let mut sp = Spectrum::new();
        let bars = sp.analyze(&[0.5_f32], 8);
        assert_eq!(bars.len(), 8);
        assert!(bars.iter().all(|&b| b.is_finite()));
    }

    #[test]
    fn log_bins_are_monotonic_and_cover_the_range() {
        let bars = 30;
        let half = 512;
        let mut prev_hi = 0_usize;
        for b in 0..bars {
            let (lo, hi) = log_bin_range(b, bars, half);
            assert!(lo < hi, "bar {b}: lo {lo} >= hi {hi}");
            assert!(hi <= half);
            assert!(lo >= prev_hi.saturating_sub(1), "bar {b} went backwards");
            prev_hi = hi;
        }
        let (_, last_hi) = log_bin_range(bars - 1, bars, half);
        assert!(last_hi >= half - 2, "top bar should reach near nyquist");
    }

    #[test]
    fn log_bins_widen_toward_high_frequencies() {
        let bars = 30;
        let half = 512;
        let (lo0, hi0) = log_bin_range(0, bars, half);
        let (lo_n, hi_n) = log_bin_range(bars - 1, bars, half);
        assert!((hi_n - lo_n) > (hi0 - lo0), "high bands must be wider");
    }

    #[test]
    fn decay_smooths_toward_zero_between_quiet_frames() {
        let mut sp = Spectrum::new();
        let loud: Vec<f32> = (0..2048)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        let first = sp.analyze(&loud, 8);
        let after = sp.analyze(&[0.0_f32; 2048], 8);
        let max_first = first.iter().cloned().fold(0.0, f32::max);
        let max_after = after.iter().cloned().fold(0.0, f32::max);
        assert!(max_after < max_first);
        assert!(max_after > 0.0);
    }
}
