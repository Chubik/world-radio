use crate::ring::SampleCons;
use ringbuf::traits::Consumer;

pub fn mix_output(
    cons_a: &mut SampleCons,
    cons_b: &mut SampleCons,
    out: &mut [f32],
    volume: f32,
    gain_a: f32,
    gain_b: f32,
) {
    let mut buf_a = vec![0.0_f32; out.len()];
    let mut buf_b = vec![0.0_f32; out.len()];
    if gain_a > 0.0 {
        cons_a.pop_slice(&mut buf_a);
    }
    if gain_b > 0.0 {
        cons_b.pop_slice(&mut buf_b);
    }
    for i in 0..out.len() {
        out[i] = volume * (buf_a[i] * gain_a + buf_b[i] * gain_b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ring::make_ring;
    use ringbuf::traits::Producer;

    #[test]
    fn mixes_single_active_slot_with_volume() {
        let (mut pa, mut ca) = make_ring(8);
        let (_pb, mut cb) = make_ring(8);
        pa.push_slice(&[1.0, 1.0, 1.0, 1.0]);
        let mut out = [0.0_f32; 4];
        mix_output(&mut ca, &mut cb, &mut out, 0.5, 1.0, 0.0);
        assert_eq!(out, [0.5, 0.5, 0.5, 0.5]);
    }

    #[test]
    fn mixes_two_slots_with_gains() {
        let (mut pa, mut ca) = make_ring(8);
        let (mut pb, mut cb) = make_ring(8);
        pa.push_slice(&[1.0, 1.0]);
        pb.push_slice(&[1.0, 1.0]);
        let mut out = [0.0_f32; 2];
        mix_output(&mut ca, &mut cb, &mut out, 1.0, 0.5, 0.5);
        assert_eq!(out, [1.0, 1.0]);
    }

    #[test]
    fn silent_slot_is_not_drained() {
        let (mut pa, mut ca) = make_ring(8);
        let (_pb, mut cb) = make_ring(8);
        pa.push_slice(&[1.0, 1.0, 1.0, 1.0]);
        let mut out = [0.0_f32; 2];
        mix_output(&mut ca, &mut cb, &mut out, 1.0, 0.0, 0.0);
        assert_eq!(out, [0.0, 0.0]);
        let mut rest = [0.0_f32; 4];
        let got = ca.pop_slice(&mut rest);
        assert_eq!(got, 4, "gain-0 slot must keep its buffered samples");
    }
}
