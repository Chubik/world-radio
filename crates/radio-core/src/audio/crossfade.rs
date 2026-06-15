pub fn ramp_gain(start: f32, target: f32, elapsed_ms: f32, dur_ms: f32) -> f32 {
    if dur_ms <= 0.0 {
        return target;
    }
    let t = (elapsed_ms / dur_ms).clamp(0.0, 1.0);
    start + (target - start) * t
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mix {
    pub gain_old: f32,
    pub gain_new: f32,
    pub done: bool,
}

pub fn crossfade_mix(elapsed_ms: f32, dur_ms: f32) -> Mix {
    if dur_ms <= 0.0 {
        return Mix {
            gain_old: 0.0,
            gain_new: 1.0,
            done: true,
        };
    }
    let gain_new = ramp_gain(0.0, 1.0, elapsed_ms, dur_ms);
    let gain_old = ramp_gain(1.0, 0.0, elapsed_ms, dur_ms);
    Mix {
        gain_old,
        gain_new,
        done: elapsed_ms >= dur_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_starts_at_start_and_ends_at_target() {
        assert_eq!(ramp_gain(0.0, 1.0, 0.0, 400.0), 0.0);
        assert_eq!(ramp_gain(0.0, 1.0, 400.0, 400.0), 1.0);
        assert_eq!(ramp_gain(1.0, 0.0, 400.0, 400.0), 0.0);
    }

    #[test]
    fn ramp_is_linear_midpoint() {
        assert!((ramp_gain(0.0, 1.0, 200.0, 400.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn ramp_clamps_past_duration() {
        assert_eq!(ramp_gain(0.0, 1.0, 800.0, 400.0), 1.0);
        assert_eq!(ramp_gain(0.0, 1.0, -50.0, 400.0), 0.0);
    }

    #[test]
    fn ramp_zero_duration_is_instant() {
        assert_eq!(ramp_gain(0.0, 1.0, 0.0, 0.0), 1.0);
    }

    #[test]
    fn mix_crossfades_old_out_new_in() {
        let m = crossfade_mix(0.0, 400.0);
        assert_eq!(m.gain_old, 1.0);
        assert_eq!(m.gain_new, 0.0);
        assert!(!m.done);

        let mid = crossfade_mix(200.0, 400.0);
        assert!((mid.gain_old - 0.5).abs() < 1e-6);
        assert!((mid.gain_new - 0.5).abs() < 1e-6);
        assert!(!mid.done);

        let end = crossfade_mix(400.0, 400.0);
        assert_eq!(end.gain_old, 0.0);
        assert_eq!(end.gain_new, 1.0);
        assert!(end.done);
    }

    #[test]
    fn mix_zero_duration_is_instant_swap() {
        let m = crossfade_mix(0.0, 0.0);
        assert_eq!(m.gain_old, 0.0);
        assert_eq!(m.gain_new, 1.0);
        assert!(m.done);
    }
}
