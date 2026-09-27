//! Incremental Server-Sent Events parser (WHATWG `text/event-stream`), fed arbitrary byte
//! chunks. Handles `\n` and `\r\n` line ends, comments, multi-line `data`, and chunk boundaries
//! anywhere (including inside a UTF-8 sequence).

/// One dispatched event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// `event:` field (`message` when absent).
    pub event: String,
    /// `data:` lines joined with `\n`.
    pub data: String,
}

/// The parser state between chunks.
#[derive(Debug, Default)]
pub struct SseParser {
    buf: Vec<u8>,
    event: Option<String>,
    data: Vec<String>,
}

impl SseParser {
    /// A fresh parser.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds `chunk`; returns the events completed by it.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        let mut start = 0;
        while let Some(pos) = self.buf[start..].iter().position(|&b| b == b'\n') {
            let end = start + pos;
            let mut line = &self.buf[start..end];
            if line.last() == Some(&b'\r') {
                line = &line[..line.len() - 1];
            }
            let line = String::from_utf8_lossy(line).into_owned();
            start = end + 1;
            if let Some(ev) = self.line(&line) {
                out.push(ev);
            }
        }
        self.buf.drain(..start);
        out
    }

    fn line(&mut self, line: &str) -> Option<SseEvent> {
        if line.is_empty() {
            let event = self.event.take();
            if self.data.is_empty() {
                return None;
            }
            let data = std::mem::take(&mut self.data).join("\n");
            return Some(SseEvent {
                event: event.unwrap_or_else(|| "message".to_owned()),
                data,
            });
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => self.event = Some(value.to_owned()),
            "data" => self.data.push(value.to_owned()),
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;

    use super::*;

    const STREAM: &str = "event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5\"}}\n\
\n\
: keep-alive comment\n\
event: ping\r\n\
data: {\"type\": \"ping\"}\r\n\
\r\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"مرحبا\"}}\n\
\n\
data: line one\n\
data: line two\n\
\n\
event: empty\n\
\n";

    fn expected() -> Vec<SseEvent> {
        vec![
            SseEvent {
                event: "message_start".into(),
                data: r#"{"type":"message_start","message":{"model":"claude-opus-5"}}"#.into(),
            },
            SseEvent {
                event: "ping".into(),
                data: r#"{"type": "ping"}"#.into(),
            },
            SseEvent {
                event: "content_block_delta".into(),
                data: r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"مرحبا"}}"#.into(),
            },
            SseEvent {
                event: "message".into(),
                data: "line one\nline two".into(),
            },
        ]
    }

    #[test]
    fn parses_events_comments_crlf_and_multiline_data() {
        let mut p = SseParser::new();
        assert_eq!(p.push(STREAM.as_bytes()), expected());
    }

    #[test]
    fn incomplete_event_waits_for_the_blank_line() {
        let mut p = SseParser::new();
        assert_eq!(p.push(b"event: a\ndata: 1\n"), vec![]);
        assert_eq!(
            p.push(b"\n"),
            vec![SseEvent {
                event: "a".into(),
                data: "1".into()
            }]
        );
    }

    proptest! {
        /// Splitting the byte stream anywhere (even inside a UTF-8 sequence) yields the same
        /// events.
        #[test]
        fn chunking_does_not_change_events(cuts in proptest::collection::vec(0usize..STREAM.len(), 0..12)) {
            let bytes = STREAM.as_bytes();
            let mut cuts = cuts;
            cuts.sort_unstable();
            let mut p = SseParser::new();
            let mut got = Vec::new();
            let mut prev = 0;
            for c in cuts.into_iter().chain([bytes.len()]) {
                got.extend(p.push(&bytes[prev..c]));
                prev = c;
            }
            prop_assert_eq!(got, expected());
        }
    }
}
