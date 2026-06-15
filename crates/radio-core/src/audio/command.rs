#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Play(String),
    Stop,
    SetVolume(f32),
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
}
