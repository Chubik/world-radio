pub fn clamp_volume(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

pub fn apply_gain(samples: &mut [f32], volume: f32) {
    let v = clamp_volume(volume);
    for s in samples.iter_mut() {
        *s *= v;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_keeps_in_range() {
        assert_eq!(clamp_volume(-0.5), 0.0);
        assert_eq!(clamp_volume(0.3), 0.3);
        assert_eq!(clamp_volume(1.7), 1.0);
    }

    #[test]
    fn apply_gain_scales_samples() {
        let mut buf = [1.0_f32, -1.0, 0.5];
        apply_gain(&mut buf, 0.5);
        assert_eq!(buf, [0.5, -0.5, 0.25]);
    }

    #[test]
    fn apply_gain_clamps_volume_first() {
        let mut buf = [1.0_f32];
        apply_gain(&mut buf, 2.0);
        assert_eq!(buf, [1.0]);
    }

    #[test]
    fn apply_gain_zero_volume_silences() {
        let mut buf = [1.0_f32, -0.5, 0.25];
        apply_gain(&mut buf, 0.0);
        assert_eq!(buf, [0.0, 0.0, 0.0]);
    }
}
