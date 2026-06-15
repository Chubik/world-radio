pub fn parse_stream_title(block: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(block);
    let start = text.find("StreamTitle='")? + "StreamTitle='".len();
    let rest = &text[start..];
    let end = rest.find("';")?;
    let title = &rest[..end];
    match title.is_empty() {
        true => None,
        false => Some(title.to_string()),
    }
}

pub struct IcyDemux {
    metaint: usize,
    audio_until_meta: usize,
    title: Option<String>,
}

impl IcyDemux {
    pub fn new(metaint: usize) -> Self {
        Self {
            metaint,
            audio_until_meta: metaint,
            title: None,
        }
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn push(&mut self, input: &[u8], audio_out: &mut Vec<u8>) {
        let mut i = 0;
        while i < input.len() {
            if self.audio_until_meta > 0 {
                let take = self.audio_until_meta.min(input.len() - i);
                audio_out.extend_from_slice(&input[i..i + take]);
                i += take;
                self.audio_until_meta -= take;
                continue;
            }
            let len_byte = input[i] as usize;
            i += 1;
            let meta_len = len_byte * 16;
            let end = (i + meta_len).min(input.len());
            if meta_len > 0 {
                if let Some(t) = parse_stream_title(&input[i..end]) {
                    self.title = Some(t);
                }
            }
            i = end;
            self.audio_until_meta = self.metaint;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_stream_title() {
        let block = b"StreamTitle='Miles Davis - So What';StreamUrl='';\0\0";
        assert_eq!(
            parse_stream_title(block),
            Some("Miles Davis - So What".to_string())
        );
    }

    #[test]
    fn no_title_returns_none() {
        let block = b"StreamUrl='http://x';\0";
        assert_eq!(parse_stream_title(block), None);
    }

    #[test]
    fn demux_strips_metadata_and_keeps_audio() {
        let mut d = IcyDemux::new(4);
        let title = b"StreamTitle='Hi';";
        let len_byte = title.len().div_ceil(16) as u8;
        let padded_len = (len_byte as usize) * 16;
        let mut meta = vec![0u8; padded_len];
        meta[..title.len()].copy_from_slice(title);

        let mut input = vec![1u8, 2, 3, 4];
        input.push(len_byte);
        input.extend_from_slice(&meta);
        input.extend_from_slice(&[5u8, 6, 7, 8]);

        let mut audio = Vec::new();
        d.push(&input, &mut audio);

        assert_eq!(audio, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(d.title(), Some("Hi"));
    }

    #[test]
    fn demux_handles_zero_length_meta_block() {
        let mut d = IcyDemux::new(2);
        let input = vec![1u8, 2, 0, 3, 4];
        let mut audio = Vec::new();
        d.push(&input, &mut audio);
        assert_eq!(audio, vec![1, 2, 3, 4]);
        assert_eq!(d.title(), None);
    }

    #[test]
    fn demux_audio_continues_across_push_calls() {
        let mut d = IcyDemux::new(6);
        let mut audio = Vec::new();
        d.push(&[1u8, 2, 3], &mut audio);
        d.push(&[4u8, 5, 6], &mut audio);
        assert_eq!(audio, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(d.title(), None);
    }
}
