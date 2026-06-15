use ringbuf::traits::Split;
use ringbuf::{HeapCons, HeapProd, HeapRb};

pub type SampleProd = HeapProd<f32>;
pub type SampleCons = HeapCons<f32>;

pub fn make_ring(capacity: usize) -> (SampleProd, SampleCons) {
    let rb = HeapRb::<f32>::new(capacity);
    rb.split()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringbuf::traits::{Consumer, Producer};

    #[test]
    fn write_then_read_roundtrips() {
        let (mut prod, mut cons) = make_ring(8);
        let wrote = prod.push_slice(&[1.0, 2.0, 3.0]);
        assert_eq!(wrote, 3);
        let mut out = [0.0_f32; 3];
        let read = cons.pop_slice(&mut out);
        assert_eq!(read, 3);
        assert_eq!(out, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn read_from_empty_returns_zero() {
        let (_prod, mut cons) = make_ring(4);
        let mut out = [0.0_f32; 4];
        assert_eq!(cons.pop_slice(&mut out), 0);
    }

    #[test]
    fn write_past_capacity_is_partial() {
        let (mut prod, _cons) = make_ring(2);
        let wrote = prod.push_slice(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(wrote, 2);
    }
}
