pub struct Resampler {
    in_rate: u32,
    out_rate: u32,
    in_ch: usize,
    out_ch: usize,
    last_frame: Vec<f32>,
    pos: f64,
    primed: bool,
}

impl Resampler {
    pub fn new(in_rate: u32, in_ch: u16, out_rate: u32, out_ch: u16) -> Resampler {
        let in_ch = in_ch.max(1) as usize;
        let out_ch = out_ch.max(1) as usize;
        Resampler {
            in_rate: in_rate.max(1),
            out_rate: out_rate.max(1),
            in_ch,
            out_ch,
            last_frame: vec![0.0; in_ch],
            pos: 0.0,
            primed: false,
        }
    }

    pub fn passthrough(&self) -> bool {
        self.in_rate == self.out_rate && self.in_ch == self.out_ch
    }

    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if self.passthrough() {
            return input.to_vec();
        }
        let frames = self.deinterleave(input);
        if frames.is_empty() {
            return Vec::new();
        }
        let resampled = self.resample_frames(&frames);
        self.map_channels(&resampled)
    }

    fn deinterleave(&self, input: &[f32]) -> Vec<Vec<f32>> {
        let n = input.len() / self.in_ch;
        let mut frames = Vec::with_capacity(n);
        for i in 0..n {
            let base = i * self.in_ch;
            frames.push(input[base..base + self.in_ch].to_vec());
        }
        frames
    }

    fn resample_frames(&mut self, frames: &[Vec<f32>]) -> Vec<Vec<f32>> {
        let ratio = self.in_rate as f64 / self.out_rate as f64;
        let prev = std::mem::take(&mut self.last_frame);
        if !self.primed {
            self.pos = 0.0;
            self.primed = true;
        }
        let mut out: Vec<Vec<f32>> = Vec::new();
        while self.pos < frames.len() as f64 {
            let base = self.pos.floor() as isize;
            let frac = self.pos - base as f64;
            let a = frame_at(base, &prev, frames);
            let b = frame_at(base + 1, &prev, frames);
            out.push(lerp_frame(a, b, frac));
            self.pos += ratio;
        }
        self.pos -= frames.len() as f64;
        self.last_frame = frames[frames.len() - 1].clone();
        out
    }

    fn map_channels(&self, frames: &[Vec<f32>]) -> Vec<f32> {
        let mut out = Vec::with_capacity(frames.len() * self.out_ch);
        for f in frames {
            map_frame(f, self.out_ch, &mut out);
        }
        out
    }
}

fn frame_at<'a>(i: isize, prev: &'a [f32], frames: &'a [Vec<f32>]) -> &'a [f32] {
    if i < 0 {
        return prev;
    }
    let i = i as usize;
    match frames.get(i) {
        Some(f) => f,
        None => &frames[frames.len() - 1],
    }
}

fn lerp_frame(a: &[f32], b: &[f32], frac: f64) -> Vec<f32> {
    let t = frac as f32;
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| x + (y - x) * t)
        .collect()
}

fn map_frame(frame: &[f32], out_ch: usize, out: &mut Vec<f32>) {
    let in_ch = frame.len();
    if in_ch == out_ch {
        out.extend_from_slice(frame);
        return;
    }
    if in_ch == 1 {
        for _ in 0..out_ch {
            out.push(frame[0]);
        }
        return;
    }
    if out_ch == 1 {
        let sum: f32 = frame.iter().sum();
        out.push(sum / in_ch as f32);
        return;
    }
    for c in 0..out_ch {
        out.push(frame[c % in_ch]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_when_rate_and_channels_match() {
        let mut r = Resampler::new(48_000, 2, 48_000, 2);
        assert!(r.passthrough());
        let out = r.process(&[0.1, 0.2, 0.3, 0.4]);
        assert_eq!(out, vec![0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn upsample_doubles_frame_count() {
        let mut r = Resampler::new(24_000, 1, 48_000, 1);
        let out = r.process(&[0.0, 1.0, 0.0, 1.0]);
        assert!(out.len() >= 7 && out.len() <= 9, "len was {}", out.len());
    }

    #[test]
    fn downsample_halves_frame_count() {
        let mut r = Resampler::new(48_000, 1, 24_000, 1);
        let input: Vec<f32> = (0..8).map(|i| i as f32).collect();
        let out = r.process(&input);
        assert!(out.len() >= 3 && out.len() <= 5, "len was {}", out.len());
    }

    #[test]
    fn mono_to_stereo_duplicates() {
        let mut r = Resampler::new(48_000, 1, 48_000, 2);
        assert!(!r.passthrough());
        let out = r.process(&[0.5, -0.5]);
        assert_eq!(out, vec![0.5, 0.5, -0.5, -0.5]);
    }

    #[test]
    fn stereo_to_mono_averages() {
        let mut r = Resampler::new(48_000, 2, 48_000, 1);
        let out = r.process(&[1.0, 0.0, 0.4, 0.6]);
        assert_eq!(out, vec![0.5, 0.5]);
    }

    #[test]
    fn empty_input_yields_empty() {
        let mut r = Resampler::new(24_000, 2, 48_000, 2);
        assert!(r.process(&[]).is_empty());
    }

    #[test]
    fn continuous_across_calls_keeps_rate_stable() {
        let mut r = Resampler::new(44_100, 1, 48_000, 1);
        let mut total = 0usize;
        for _ in 0..100 {
            let chunk: Vec<f32> = (0..441).map(|i| (i as f32) * 0.001).collect();
            total += r.process(&chunk).len();
        }
        let approx = 48_000usize;
        let diff = (total as i64 - approx as i64).abs();
        assert!(diff < 200, "total {total} expected ~{approx}");
    }
}
