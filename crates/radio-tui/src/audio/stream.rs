use radio_core::audio::icy::IcyDemux;
use std::collections::VecDeque;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};
use symphonia::core::io::MediaSource;

pub struct IcyStream {
    resp: reqwest::blocking::Response,
    demux: Option<IcyDemux>,
    shared_title: Arc<Mutex<Option<String>>>,
}

pub struct IcyMediaSource {
    inner: IcyStream,
    pending: VecDeque<u8>,
    scratch: Vec<u8>,
}

impl IcyMediaSource {
    pub fn new(inner: IcyStream) -> Self {
        Self {
            inner,
            pending: VecDeque::new(),
            scratch: vec![0u8; 16384],
        }
    }

    pub fn shared_title(&self) -> Arc<Mutex<Option<String>>> {
        Arc::clone(&self.inner.shared_title)
    }
}

impl Read for IcyMediaSource {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        while self.pending.is_empty() {
            let mut scratch = std::mem::take(&mut self.scratch);
            let mut chunk = Vec::new();
            let n = self
                .inner
                .read_audio(&mut scratch, &mut chunk)
                .map_err(|e| io::Error::other(e.to_string()))?;
            self.scratch = scratch;
            self.pending.extend(chunk);
            if n == 0 {
                return Ok(0);
            }
        }
        let take = self.pending.len().min(out.len());
        for slot in out.iter_mut().take(take) {
            *slot = self.pending.pop_front().unwrap();
        }
        Ok(take)
    }
}

impl Seek for IcyMediaSource {
    fn seek(&mut self, _pos: SeekFrom) -> io::Result<u64> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "icy stream is not seekable",
        ))
    }
}

unsafe impl Sync for IcyMediaSource {}

impl MediaSource for IcyMediaSource {
    fn is_seekable(&self) -> bool {
        false
    }
    fn byte_len(&self) -> Option<u64> {
        None
    }
}

pub fn open(url: &str) -> anyhow::Result<IcyStream> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("world-radio/0.1")
        .build()?;
    let resp = client
        .get(url)
        .header("Icy-MetaData", "1")
        .send()?
        .error_for_status()?;
    let metaint = resp
        .headers()
        .get("icy-metaint")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok());
    let demux = metaint.map(IcyDemux::new);
    Ok(IcyStream {
        resp,
        demux,
        shared_title: Arc::new(Mutex::new(None)),
    })
}

impl IcyStream {
    pub fn title(&self) -> Option<&str> {
        self.demux.as_ref().and_then(|d| d.title())
    }

    pub fn read_audio(
        &mut self,
        scratch: &mut [u8],
        audio_out: &mut Vec<u8>,
    ) -> anyhow::Result<usize> {
        let n = self.resp.read(scratch)?;
        if n == 0 {
            return Ok(0);
        }
        let prev_title = self.title().map(str::to_string);
        match self.demux.as_mut() {
            Some(d) => d.push(&scratch[..n], audio_out),
            None => audio_out.extend_from_slice(&scratch[..n]),
        }
        let new_title = self.title().map(str::to_string);
        if new_title != prev_title {
            if let Ok(mut lock) = self.shared_title.lock() {
                *lock = new_title;
            }
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_reads_metaint_header() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/stream")
            .with_status(200)
            .with_header("icy-metaint", "16000")
            .with_body(vec![0u8; 32])
            .create();
        let s = open(&format!("{}/stream", server.url())).unwrap();
        assert!(s.demux.is_some());
    }

    #[test]
    fn open_without_metaint_has_no_demux() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/plain")
            .with_status(200)
            .with_body(vec![0u8; 32])
            .create();
        let s = open(&format!("{}/plain", server.url())).unwrap();
        assert!(s.demux.is_none());
    }

    #[test]
    fn read_audio_passes_through_when_no_metadata() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/plain")
            .with_status(200)
            .with_body(vec![7u8; 10])
            .create();
        let mut s = open(&format!("{}/plain", server.url())).unwrap();
        let mut scratch = [0u8; 4];
        let mut audio = Vec::new();
        let mut total = 0;
        loop {
            let n = s.read_audio(&mut scratch, &mut audio).unwrap();
            if n == 0 {
                break;
            }
            total += n;
        }
        assert_eq!(total, 10);
        assert_eq!(audio, vec![7u8; 10]);
    }
}
