pub mod output;
pub mod ring;
pub mod slot;
pub mod stream;

pub use radio_core::audio::command::{Command, Status};

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

pub(crate) fn volume_to_bits(v: f32) -> u32 {
    radio_core::audio::gain::clamp_volume(v).to_bits()
}

pub(crate) fn bits_to_volume(b: u32) -> f32 {
    f32::from_bits(b)
}

pub struct SharedVolume(Arc<AtomicU32>);

impl SharedVolume {
    pub fn new(initial: f32) -> Self {
        Self(Arc::new(AtomicU32::new(volume_to_bits(initial))))
    }
    pub fn get(&self) -> f32 {
        bits_to_volume(self.0.load(Ordering::Relaxed))
    }
    pub fn set(&self, v: f32) {
        self.0.store(volume_to_bits(v), Ordering::Relaxed);
    }
    pub fn clone_handle(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

pub struct SharedGain(Arc<AtomicU32>);

impl SharedGain {
    pub fn new(initial: f32) -> Self {
        Self(Arc::new(AtomicU32::new(initial.to_bits())))
    }
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
    pub fn set(&self, v: f32) {
        self.0.store(v.to_bits(), Ordering::Relaxed);
    }
    pub fn clone_handle(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

use crate::audio::ring::SampleCons;

pub struct AudioEngine {
    cmd_tx: Sender<Command>,
    status_rx: Receiver<Status>,
    volume: SharedVolume,
    crossfade_on: Arc<AtomicBool>,
    tap: std::sync::Mutex<SampleCons>,
    stop: Arc<AtomicBool>,
    done_rx: Receiver<()>,
}

const RING_CAP: usize = 48_000 * 2 * 2;

impl AudioEngine {
    pub fn spawn() -> anyhow::Result<Self> {
        use crate::audio::output::mix_output;
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        use std::sync::mpsc;
        use std::thread;

        let (cmd_tx, cmd_rx) = mpsc::channel::<Command>();
        let (status_tx, status_rx) = mpsc::channel::<Status>();
        let (done_tx, done_rx) = mpsc::channel::<()>();
        let volume = SharedVolume::new(0.7);
        let crossfade_on = Arc::new(AtomicBool::new(true));

        let (prod_a, cons_a) = ring::make_ring(RING_CAP);
        let (prod_b, cons_b) = ring::make_ring(RING_CAP);
        let (tap_prod, tap_cons) = ring::make_ring(48_000);

        let gain_a = SharedGain::new(0.0);
        let gain_b = SharedGain::new(0.0);

        let cb_volume = volume.clone_handle();
        let cb_gain_a = gain_a.clone_handle();
        let cb_gain_b = gain_b.clone_handle();
        let cons_a = Arc::new(std::sync::Mutex::new(cons_a));
        let cons_b = Arc::new(std::sync::Mutex::new(cons_b));
        let cb_cons_a = Arc::clone(&cons_a);
        let cb_cons_b = Arc::clone(&cons_b);

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("no output device"))?;
        let config = device.default_output_config()?;
        let stream_config: cpal::StreamConfig = config.clone().into();
        let out_rate = stream_config.sample_rate;
        let out_channels = stream_config.channels;

        let out_stream = device.build_output_stream(
            &stream_config,
            move |data: &mut [f32], _| {
                let mut ca = cb_cons_a.lock().unwrap();
                let mut cb = cb_cons_b.lock().unwrap();
                mix_output(
                    &mut ca,
                    &mut cb,
                    data,
                    cb_volume.get(),
                    cb_gain_a.get(),
                    cb_gain_b.get(),
                );
            },
            move |err| crate::log_warn!("output stream error: {err}"),
            None,
        )?;
        out_stream.play()?;

        let stop = Arc::new(AtomicBool::new(false));
        let ctl = slot::Controller::new(
            slot::ControllerCfg {
                out_rate,
                out_channels,
                crossfade_on: Arc::clone(&crossfade_on),
            },
            [
                slot::SlotIo::new(prod_a, Arc::clone(&cons_a), gain_a),
                slot::SlotIo::new(prod_b, Arc::clone(&cons_b), gain_b),
            ],
            tap_prod,
        );

        let thread_stop = Arc::clone(&stop);
        thread::spawn(move || {
            let _keep_stream = out_stream;
            ctl.run(&cmd_rx, &status_tx, &thread_stop);
            let _ = done_tx.send(());
        });

        Ok(Self {
            cmd_tx,
            status_rx,
            volume,
            crossfade_on,
            tap: std::sync::Mutex::new(tap_cons),
            stop,
            done_rx,
        })
    }

    pub fn play(&self, url: &str) {
        let _ = self.cmd_tx.send(Command::Play(url.to_string()));
    }
    pub fn stop(&self) {
        let _ = self.cmd_tx.send(Command::Stop);
    }
    pub fn set_volume(&self, v: f32) {
        let clamped = radio_core::audio::gain::clamp_volume(v);
        self.volume.set(clamped);
        let _ = self.cmd_tx.send(Command::SetVolume(clamped));
    }
    pub fn set_crossfade(&self, on: bool) {
        self.crossfade_on.store(on, Ordering::Relaxed);
    }
    pub fn poll_status(&self) -> Option<Status> {
        self.status_rx.try_recv().ok()
    }
    pub fn read_tap(&self, out: &mut [f32]) -> usize {
        use ringbuf::traits::Consumer;
        let mut tap = self.tap.lock().unwrap();
        tap.pop_slice(out)
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.cmd_tx.send(Command::Stop);
        let _ = self.done_rx.recv_timeout(std::time::Duration::from_secs(3));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_volume_clamps_and_shares() {
        let v = SharedVolume::new(0.5);
        let handle = v.clone_handle();
        v.set(2.0);
        assert_eq!(handle.get(), 1.0);
        v.set(-1.0);
        assert_eq!(handle.get(), 0.0);
    }

    #[test]
    fn volume_bits_roundtrip() {
        assert_eq!(bits_to_volume(volume_to_bits(0.3)), 0.3);
    }

    #[test]
    fn shared_gain_shares_value() {
        let g = SharedGain::new(0.0);
        let h = g.clone_handle();
        g.set(0.5);
        assert_eq!(h.get(), 0.5);
    }

    #[test]
    #[ignore]
    fn live_switching_between_real_streams() {
        let engine = match AudioEngine::spawn() {
            Ok(e) => e,
            Err(_) => return,
        };
        let url1 = "https://0n-jazz.radionetz.de/0n-jazz.mp3";
        let url2 = "https://stream.laut.fm/jazz";

        eprintln!("=== live: play url1 ===");
        engine.play(url1);
        for _ in 0..30 {
            if let Some(s) = engine.poll_status() {
                eprintln!("status: {s:?}");
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        eprintln!("=== live: switch to url2 (crossfade) ===");
        engine.play(url2);
        for _ in 0..40 {
            if let Some(s) = engine.poll_status() {
                eprintln!("status: {s:?}");
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        eprintln!("=== live: done ===");
    }

    #[test]
    fn switching_play_emits_new_buffering() {
        let engine = match AudioEngine::spawn() {
            Ok(e) => e,
            Err(_) => return,
        };
        engine.play("http://127.0.0.1:0/never-resolves");
        let mut seen_first_buffering = false;
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_millis(500) {
            if let Some(Status::Buffering) = engine.poll_status() {
                seen_first_buffering = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(seen_first_buffering, "first Play should emit Buffering");

        engine.play("http://127.0.0.1:0/also-fails");
        let mut seen_second_buffering = false;
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_millis(2000) {
            if let Some(Status::Buffering) = engine.poll_status() {
                seen_second_buffering = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            seen_second_buffering,
            "second Play should also emit Buffering — switching is reachable"
        );
    }
}
