use crate::ring::{SampleCons, SampleProd};
use crate::{Command, SharedGain, Status};
use radio_core::audio::crossfade::crossfade_mix;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const CROSSFADE_MS: f32 = 450.0;
const TICK_MS: u64 = 10;

pub struct ControllerCfg {
    pub out_rate: u32,
    pub out_channels: u16,
    pub crossfade_on: Arc<AtomicBool>,
}

pub struct SlotIo {
    prod: Arc<Mutex<SampleProd>>,
    cons: Arc<Mutex<SampleCons>>,
    gain: SharedGain,
}

impl SlotIo {
    pub fn new(prod: SampleProd, cons: Arc<Mutex<SampleCons>>, gain: SharedGain) -> Self {
        Self {
            prod: Arc::new(Mutex::new(prod)),
            cons,
            gain,
        }
    }

    fn drain(&self) {
        use ringbuf::traits::Consumer;
        if let Ok(mut c) = self.cons.lock() {
            c.clear();
        }
    }
}

/// A running decoder thread. Aborting it NEVER blocks the controller: it only
/// flips the abort flag and hands the JoinHandle to the cemetery, which is reaped
/// non-blockingly. The thread winds down on its own once it leaves the network
/// call it is stuck in.
struct DecodeHandle {
    abort: Arc<AtomicBool>,
    ready_rx: Receiver<()>,
    join: Option<JoinHandle<()>>,
    ready_seen: bool,
}

impl DecodeHandle {
    /// Signal the thread to stop and return its JoinHandle for the cemetery.
    /// Does NOT join — the controller must never block on a stuck network read.
    fn abort(mut self) -> Option<JoinHandle<()>> {
        self.abort.store(true, Ordering::Relaxed);
        self.join.take()
    }
}

/// An active crossfade in progress (non-blocking; stepped each controller tick).
struct Fade {
    start: Instant,
    dur_ms: f32,
    new_slot: usize,
    old_slot: Option<usize>,
}

pub struct Controller {
    cfg: ControllerCfg,
    slots: [SlotIo; 2],
    tap_prod: Arc<Mutex<SampleProd>>,
    active: Option<(usize, DecodeHandle)>,
    incoming: Option<(usize, DecodeHandle)>,
    fade: Option<Fade>,
    cemetery: Vec<JoinHandle<()>>,
}

impl Controller {
    pub fn new(cfg: ControllerCfg, slots: [SlotIo; 2], tap_prod: SampleProd) -> Self {
        Self {
            cfg,
            slots,
            tap_prod: Arc::new(Mutex::new(tap_prod)),
            active: None,
            incoming: None,
            fade: None,
            cemetery: Vec::new(),
        }
    }

    pub fn run(
        mut self,
        cmd_rx: &Receiver<Command>,
        status_tx: &Sender<Status>,
        stop: &AtomicBool,
    ) {
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            while let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    Command::Play(url) => self.start_incoming(&url, status_tx),
                    Command::Stop => self.stop_all(),
                    Command::SetVolume(_) => {}
                }
            }
            self.step_fade();
            self.poll_incoming(status_tx);
            self.reap();
            std::thread::sleep(Duration::from_millis(TICK_MS));
        }
        self.shutdown();
    }

    fn free_slot(&self) -> usize {
        match self.active {
            Some((i, _)) => 1 - i,
            None => 0,
        }
    }

    fn bury(&mut self, handle: DecodeHandle) {
        if let Some(j) = handle.abort() {
            self.cemetery.push(j);
        }
    }

    fn reap(&mut self) {
        let mut i = 0;
        while i < self.cemetery.len() {
            if self.cemetery[i].is_finished() {
                let j = self.cemetery.swap_remove(i);
                let _ = j.join();
            } else {
                i += 1;
            }
        }
    }

    fn start_incoming(&mut self, url: &str, status_tx: &Sender<Status>) {
        // a newer Play supersedes a still-pending incoming one
        if let Some((_, h)) = self.incoming.take() {
            self.bury(h);
        }
        // also cancel an in-progress fade — its new slot is no longer the target
        self.fade = None;
        let slot = self.free_slot();
        self.slots[slot].gain.set(0.0);
        self.slots[slot].drain();
        let _ = status_tx.send(Status::Buffering);
        let handle = spawn_decode(
            url.to_string(),
            Arc::clone(&self.slots[slot].prod),
            Arc::clone(&self.tap_prod),
            self.cfg.out_rate,
            self.cfg.out_channels,
            status_tx.clone(),
        );
        self.incoming = Some((slot, handle));
    }

    fn poll_incoming(&mut self, _status_tx: &Sender<Status>) {
        let Some((_, handle)) = self.incoming.as_mut() else {
            return;
        };
        if !handle.ready_seen && handle.ready_rx.try_recv().is_ok() {
            handle.ready_seen = true;
        }
        if !handle.ready_seen {
            self.check_active_finished();
            return;
        }
        // incoming produced audio — promote it and start the crossfade
        let (incoming_slot, incoming_handle) = self.incoming.take().unwrap();
        let old = self.active.take();
        let old_slot = old.as_ref().map(|(i, _)| *i);
        let crossfade = self.cfg.crossfade_on.load(Ordering::Relaxed);
        let dur = match crossfade {
            true => CROSSFADE_MS,
            false => 0.0,
        };
        self.fade = Some(Fade {
            start: Instant::now(),
            dur_ms: dur,
            new_slot: incoming_slot,
            old_slot,
        });
        self.active = Some((incoming_slot, incoming_handle));
        if let Some((_, h)) = old {
            self.bury(h);
        }
        // apply the first fade frame immediately
        self.step_fade();
    }

    fn step_fade(&mut self) {
        let Some(fade) = self.fade.as_ref() else {
            return;
        };
        let elapsed = fade.start.elapsed().as_secs_f32() * 1000.0;
        let mix = crossfade_mix(elapsed, fade.dur_ms);
        self.slots[fade.new_slot].gain.set(mix.gain_new);
        if let Some(o) = fade.old_slot {
            self.slots[o].gain.set(mix.gain_old);
        }
        if mix.done {
            self.slots[fade.new_slot].gain.set(1.0);
            if let Some(o) = fade.old_slot {
                self.slots[o].gain.set(0.0);
            }
            self.fade = None;
        }
    }

    fn check_active_finished(&mut self) {
        let finished = match &self.active {
            Some((_, h)) => h.join.as_ref().is_some_and(|j| j.is_finished()),
            None => false,
        };
        if finished {
            if let Some((slot, h)) = self.active.take() {
                self.slots[slot].gain.set(0.0);
                self.bury(h);
            }
        }
    }

    fn stop_all(&mut self) {
        self.fade = None;
        if let Some((slot, h)) = self.active.take() {
            self.slots[slot].gain.set(0.0);
            self.bury(h);
        }
        if let Some((slot, h)) = self.incoming.take() {
            self.slots[slot].gain.set(0.0);
            self.bury(h);
        }
    }

    /// Final teardown on engine shutdown — here we CAN block, briefly, to let
    /// threads exit cleanly before the process moves on.
    fn shutdown(&mut self) {
        self.stop_all();
        for j in self.cemetery.drain(..) {
            let _ = j.join();
        }
    }
}

fn spawn_decode(
    url: String,
    prod: Arc<Mutex<SampleProd>>,
    tap_prod: Arc<Mutex<SampleProd>>,
    out_rate: u32,
    out_channels: u16,
    status_tx: Sender<Status>,
) -> DecodeHandle {
    let abort = Arc::new(AtomicBool::new(false));
    let abort_thread = Arc::clone(&abort);
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
    let join = std::thread::spawn(move || {
        if let Err(e) = decode_slot(
            &url,
            &prod,
            &tap_prod,
            out_rate,
            out_channels,
            &status_tx,
            &abort_thread,
            &ready_tx,
        ) {
            if !abort_thread.load(Ordering::Relaxed) {
                let _ = status_tx.send(Status::Error(e.to_string()));
            }
        }
    });
    DecodeHandle {
        abort,
        ready_rx,
        join: Some(join),
        ready_seen: false,
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_slot(
    url: &str,
    prod: &Arc<Mutex<SampleProd>>,
    tap_prod: &Arc<Mutex<SampleProd>>,
    out_rate: u32,
    out_channels: u16,
    status_tx: &Sender<Status>,
    abort: &AtomicBool,
    ready_tx: &Sender<()>,
) -> anyhow::Result<()> {
    use crate::stream;
    use radio_core::audio::resample::Resampler;
    use ringbuf::traits::Producer;
    use symphonia::core::codecs::audio::AudioDecoderOptions;
    use symphonia::core::errors::Error as SymphoniaError;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;

    if abort.load(Ordering::Relaxed) {
        return Ok(());
    }
    let icy = stream::open(url)?;
    if abort.load(Ordering::Relaxed) {
        return Ok(());
    }
    let source = stream::IcyMediaSource::new(icy);
    let shared_title = source.shared_title();
    let mss = MediaSourceStream::new(Box::new(source), Default::default());
    let mut format = symphonia::default::get_probe()
        .probe(
            &Hint::new(),
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| anyhow::anyhow!("probe failed: {e}"))?;
    if abort.load(Ordering::Relaxed) {
        return Ok(());
    }

    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| anyhow::anyhow!("no audio track"))?;
    let track_id = track.id;
    let codec_params = track
        .codec_params
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no codec params"))?
        .audio()
        .ok_or_else(|| anyhow::anyhow!("no audio codec params"))?
        .clone();

    let sample_rate = codec_params.sample_rate.unwrap_or(44100);
    let channels = codec_params
        .channels
        .as_ref()
        .map(|c| c.count())
        .unwrap_or(2) as u16;

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&codec_params, &AudioDecoderOptions::default())
        .map_err(|e| anyhow::anyhow!("decoder init failed: {e}"))?;

    let mut resampler = Resampler::new(sample_rate, channels, out_rate, out_channels);
    let mut announced = false;
    let mut last_title: Option<String> = None;
    loop {
        if abort.load(Ordering::Relaxed) {
            return Ok(());
        }

        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => return Ok(()),
            Err(SymphoniaError::ResetRequired) => return Ok(()),
            Err(_) => return Ok(()),
        };
        if packet.track_id != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphoniaError::IoError(_)) => continue,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(_) => return Ok(()),
        };

        if !announced {
            let _ = ready_tx.send(());
            let _ = status_tx.send(Status::Playing {
                sample_rate,
                channels,
                title: None,
            });
            announced = true;
        }

        if let Ok(t) = shared_title.try_lock() {
            let new = t.clone();
            drop(t);
            if new != last_title {
                last_title = new.clone();
                let _ = status_tx.send(Status::Playing {
                    sample_rate,
                    channels,
                    title: new,
                });
            }
        }

        let mut inter: Vec<f32> = Vec::new();
        decoded.copy_to_vec_interleaved(&mut inter);
        let out = resampler.process(&inter);

        let mut off = 0;
        while off < out.len() {
            if abort.load(Ordering::Relaxed) {
                return Ok(());
            }
            let wrote = {
                let mut p = prod.lock().unwrap();
                p.push_slice(&out[off..])
            };
            if wrote > 0 {
                let tap_end = (off + wrote).min(out.len());
                if let Ok(mut tp) = tap_prod.lock() {
                    tp.push_slice(&out[off..tap_end]);
                }
                off += wrote;
            } else {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abort_is_non_blocking() {
        // A handle whose thread sleeps for a long time must abort instantly.
        let abort = Arc::new(AtomicBool::new(false));
        let abort_t = Arc::clone(&abort);
        let (_ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
        let join = std::thread::spawn(move || {
            for _ in 0..200 {
                if abort_t.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        let h = DecodeHandle {
            abort,
            ready_rx,
            join: Some(join),
            ready_seen: false,
        };
        let start = Instant::now();
        let reaped = h.abort();
        // abort() must return immediately, not wait for the thread
        assert!(
            start.elapsed() < Duration::from_millis(50),
            "abort() blocked for {:?}",
            start.elapsed()
        );
        // the returned thread eventually finishes once it sees the flag
        if let Some(j) = reaped {
            let _ = j.join();
        }
    }
}
