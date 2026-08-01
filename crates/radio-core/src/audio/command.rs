#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Play(String),
    Stop,
    SetVolume(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// the origin answered and the answer was fatal — blame the station
    StreamDead,
    /// no usable connectivity — blame the network, never the station
    NetworkDown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Idle,
    Buffering,
    Playing {
        sample_rate: u32,
        channels: u16,
        title: Option<String>,
    },
    Retrying(u32),
    Error(String),
    StreamError {
        message: String,
        kind: FailureKind,
    },
}

/// unknown causes deliberately fall through to NetworkDown: under-hiding is
/// recoverable, mass-hiding live stations is not.
pub fn classify_failure(err: &anyhow::Error) -> FailureKind {
    let text = format!("{err:#}").to_ascii_lowercase();
    // symphonia's probe() itself (not a codec) raises this exact Unsupported text
    // when the body parses as something with no known container format at all —
    // e.g. HTTP 200 + HTML from a captive portal, hotel/airport wifi splash, or ISP
    // DNS hijack. That is the single most common "my network is broken" case, and
    // it must never be confused with a genuinely unsupported CODEC (which instead
    // names the codec, e.g. "unsupported feature: mp3 layer 0", and stays below).
    if text.contains("no suitable format reader found") {
        return FailureKind::NetworkDown;
    }
    const NETWORK: [&str; 5] = [
        "timed out",
        "network is unreachable",
        "no route to host",
        "network is down",
        // mid-stream TCP reset (router restart, wifi hiccup, ISP NAT timeout) — a
        // healthy station's connection can be reset by the local path, so this is
        // never proof the origin is dead.
        "connection reset",
    ];
    const STREAM: [&str; 6] = [
        "http status",
        "connection refused",
        "dns error",
        // matches symphonia's `Error::Unsupported` display ("unsupported feature: ...")
        "unsupported",
        // matches symphonia's `Error::DecodeError` display ("malformed stream: {msg}"),
        // e.g. a bad mp3 header — codec-specific, no network path produces this text.
        "malformed stream",
        // exact text symphonia's `read_buf_exact` uses for a short header read
        // (io/buf_reader.rs) — a truncated/garbage stream, not a network symptom.
        "buffer underrun",
    ];
    if NETWORK.iter().any(|n| text.contains(n)) {
        return FailureKind::NetworkDown;
    }
    match STREAM.iter().any(|s| text.contains(s)) {
        true => FailureKind::StreamDead,
        false => FailureKind::NetworkDown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_carries_url() {
        let c = Command::Play("http://x/stream".into());
        assert_eq!(c, Command::Play("http://x/stream".into()));
    }

    #[test]
    fn playing_status_carries_format() {
        let s = Status::Playing {
            sample_rate: 44100,
            channels: 2,
            title: None,
        };
        assert_eq!(
            s,
            Status::Playing {
                sample_rate: 44100,
                channels: 2,
                title: None,
            }
        );
    }

    #[test]
    fn classifies_http_status_as_stream_dead() {
        let e = anyhow::anyhow!("HTTP status client error (404 Not Found) for url (http://x/s)");
        assert_eq!(classify_failure(&e), FailureKind::StreamDead);
    }

    #[test]
    fn classifies_connection_refused_as_stream_dead() {
        let e = anyhow::anyhow!("tcp connect error: Connection refused (os error 61)");
        assert_eq!(classify_failure(&e), FailureKind::StreamDead);
    }

    #[test]
    fn classifies_dns_failure_as_stream_dead() {
        let e = anyhow::anyhow!(
            "dns error: failed to lookup address information: nodename nor servname provided"
        );
        assert_eq!(classify_failure(&e), FailureKind::StreamDead);
    }

    #[test]
    fn classifies_unsupported_format_as_stream_dead() {
        let e = anyhow::anyhow!("unsupported codec: core (format) error");
        assert_eq!(classify_failure(&e), FailureKind::StreamDead);
    }

    #[test]
    fn classifies_timeout_as_network_down() {
        let e = anyhow::anyhow!("operation timed out");
        assert_eq!(classify_failure(&e), FailureKind::NetworkDown);
    }

    #[test]
    fn classifies_unreachable_network_as_network_down() {
        let e = anyhow::anyhow!("tcp connect error: Network is unreachable (os error 51)");
        assert_eq!(classify_failure(&e), FailureKind::NetworkDown);
    }

    #[test]
    fn classifies_unknown_error_as_network_down() {
        // unknown causes must default to the safe direction: do not blame the station
        let e = anyhow::anyhow!("something we have never seen");
        assert_eq!(classify_failure(&e), FailureKind::NetworkDown);
    }

    #[test]
    fn classifies_connection_reset_as_network_down() {
        // a mid-stream reset (router restart, wifi hiccup, ISP NAT timeout) is a
        // local-network symptom, not proof the origin died — must under-hide.
        let e = anyhow::anyhow!("connection reset by peer");
        assert_eq!(classify_failure(&e), FailureKind::NetworkDown);
    }

    #[test]
    fn classifies_symphonia_unsupported_feature_as_stream_dead() {
        let e = anyhow::anyhow!("unsupported feature: mp3 layer 0");
        assert_eq!(classify_failure(&e), FailureKind::StreamDead);
    }

    #[test]
    fn classifies_network_error_mentioning_format_as_network_down() {
        // the bare word "format" must never win over a genuine network cause
        let e = anyhow::anyhow!("network is unreachable while negotiating stream format");
        assert_eq!(classify_failure(&e), FailureKind::NetworkDown);
    }

    #[test]
    fn classifies_truncated_header_probe_failure_as_stream_dead() {
        // the exact string slot.rs produces when probe() bottoms out reading a
        // fixed-size header short: symphonia's read_buf_exact UnexpectedEof.
        let e = anyhow::anyhow!("probe failed: buffer underrun");
        assert_eq!(classify_failure(&e), FailureKind::StreamDead);
    }

    #[test]
    fn classifies_malformed_stream_probe_failure_as_stream_dead() {
        // the exact wrapping slot.rs produces around a symphonia DecodeError.
        let e = anyhow::anyhow!("probe failed: malformed stream: invalid header");
        assert_eq!(classify_failure(&e), FailureKind::StreamDead);
    }

    #[test]
    fn classifies_captive_portal_no_format_reader_as_network_down() {
        // verified against symphonia-core 0.6.0: probe.rs:597 calls
        // unsupported_error("core (probe): no suitable format reader found"),
        // whose Display prefixes "unsupported feature: " — this exact text is what
        // slot.rs's "probe failed: {e}" wrap produces when the body parses as
        // HTTP 200 + HTML (captive portal, hotel/airport wifi splash, ISP DNS
        // hijack). Not a codec failure — must never be blamed on the station.
        let e = anyhow::anyhow!(
            "probe failed: unsupported feature: core (probe): no suitable format reader found"
        );
        assert_eq!(classify_failure(&e), FailureKind::NetworkDown);
    }

    #[test]
    fn classifies_codec_level_unsupported_feature_as_stream_dead() {
        // a genuinely unsupported codec from make_audio_decoder ("decoder init
        // failed: unsupported feature: <codec>") is a real dead-stream signal and
        // must stay StreamDead even though the probe's no-format-reader case (above)
        // now returns NetworkDown.
        let e = anyhow::anyhow!("decoder init failed: unsupported feature: mp3 layer 0");
        assert_eq!(classify_failure(&e), FailureKind::StreamDead);
    }

    #[test]
    fn stream_error_status_carries_kind() {
        let s = Status::StreamError {
            message: "boom".into(),
            kind: FailureKind::StreamDead,
        };
        assert_eq!(
            s,
            Status::StreamError {
                message: "boom".into(),
                kind: FailureKind::StreamDead,
            }
        );
    }
}
