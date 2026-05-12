//! Incremental SSE stream parser for dialectic streaming.
//!
//! Byte-stream level parser that handles split UTF-8 codepoints and split lines
//! across arbitrary network chunk boundaries. Parity with Python `sse.py`.

use serde_json::Value;

/// Incremental SSE parser that extracts `delta.content` strings from a
/// `data: <json>` line stream.
///
/// Each `data:` line is parsed as an independent JSON object (not concatenated
/// multi-line). Lines are split on `\n`, `\r`, or `\r\n`.
pub(crate) struct SseParser {
    buffer: String,
    pending_bytes: Vec<u8>,
    done: bool,
}

#[expect(dead_code)]
impl SseParser {
    /// Create a new parser.
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            pending_bytes: Vec::new(),
            done: false,
        }
    }

    /// Feed a byte chunk from the SSE stream, returning extracted content strings.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<String> {
        if self.done || chunk.is_empty() {
            return Vec::new();
        }
        self.decode_pending(chunk);
        self.drain_lines(false)
    }

    /// Flush remaining bytes and return any final content strings.
    pub fn finalize(&mut self) -> Vec<String> {
        if self.done {
            return Vec::new();
        }
        if !self.pending_bytes.is_empty() {
            let lossy = String::from_utf8_lossy(&self.pending_bytes);
            self.buffer.push_str(&lossy);
            self.pending_bytes.clear();
        }
        self.drain_lines(true)
    }

    /// Whether the stream has emitted a `done: true` message.
    #[must_use]
    pub fn done(&self) -> bool {
        self.done
    }

    fn decode_pending(&mut self, chunk: &[u8]) {
        self.pending_bytes.extend_from_slice(chunk);
        match std::str::from_utf8(&self.pending_bytes) {
            Ok(s) => {
                self.buffer.push_str(s);
                self.pending_bytes.clear();
            }
            Err(e) => {
                let valid_up_to = e.valid_up_to();
                if valid_up_to > 0 {
                    let valid =
                        std::str::from_utf8(&self.pending_bytes[..valid_up_to]).unwrap_or_default();
                    self.buffer.push_str(valid);
                    self.pending_bytes = self.pending_bytes[valid_up_to..].to_vec();
                }
            }
        }
    }

    fn drain_lines(&mut self, flush_partial: bool) -> Vec<String> {
        let mut results = Vec::new();
        while !self.done {
            let Some(line) = self.pop_line(flush_partial) else {
                break;
            };
            if let Some(content) = self.handle_line(&line) {
                results.push(content);
            }
        }
        results
    }

    fn pop_line(&mut self, flush_partial: bool) -> Option<String> {
        if self.buffer.is_empty() {
            return None;
        }

        let idx_n = self.buffer.find('\n');
        let idx_r = self.buffer.find('\r');

        match (idx_n, idx_r) {
            (None, None) => {
                if flush_partial {
                    Some(std::mem::take(&mut self.buffer))
                } else {
                    None
                }
            }
            (Some(n), None) => {
                let line = self.buffer[..n].to_string();
                self.buffer = self.buffer[n + 1..].to_string();
                Some(line.strip_suffix('\r').map(str::to_string).unwrap_or(line))
            }
            (None, Some(r)) => {
                if r == self.buffer.len() - 1 && !flush_partial {
                    return None;
                }
                if r + 1 < self.buffer.len() && self.buffer.as_bytes()[r + 1] == b'\n' {
                    let line = self.buffer[..r].to_string();
                    self.buffer = self.buffer[r + 2..].to_string();
                    return Some(line);
                }
                let line = self.buffer[..r].to_string();
                self.buffer = self.buffer[r + 1..].to_string();
                Some(line)
            }
            (Some(n), Some(r)) => {
                if r < n {
                    if r + 1 < self.buffer.len() && self.buffer.as_bytes()[r + 1] == b'\n' {
                        let line = self.buffer[..r].to_string();
                        self.buffer = self.buffer[r + 2..].to_string();
                        return Some(line);
                    }
                    let line = self.buffer[..r].to_string();
                    self.buffer = self.buffer[r + 1..].to_string();
                    Some(line)
                } else {
                    let line = self.buffer[..n].to_string();
                    self.buffer = self.buffer[n + 1..].to_string();
                    Some(line.strip_suffix('\r').map(str::to_string).unwrap_or(line))
                }
            }
        }
    }

    fn handle_line(&mut self, line: &str) -> Option<String> {
        let rest = line.strip_prefix("data:")?;
        let json_str = rest.trim_start_matches(' ');
        if json_str.is_empty() {
            return None;
        }

        let parsed: Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(e) => {
                #[cfg(feature = "tracing")]
                tracing::warn!(
                    "Failed to decode streaming chunk: {} (data: {})",
                    e,
                    &json_str[..json_str.len().min(100)]
                );
                let _ = &e;
                return None;
            }
        };

        let obj = parsed.as_object()?;

        if obj.get("done").and_then(Value::as_bool).unwrap_or(false) {
            self.done = true;
            return None;
        }

        let delta = obj.get("delta")?.as_object()?;
        let content = delta.get("content")?;
        match content.as_str() {
            Some(s) if !s.is_empty() => Some(s.to_string()),
            _ => None,
        }
    }
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn data_line(json: &str) -> Vec<u8> {
        format!("data: {json}\n\n").into_bytes()
    }

    #[test]
    fn f8_2_2_basic_data_line() {
        let mut p = SseParser::new();
        let r = p.feed(&data_line(r#"{"delta":{"content":"hello"}}"#));
        assert_eq!(r, vec!["hello"]);
        assert!(!p.done());
    }

    #[test]
    fn f8_2_3_done_flag() {
        let mut p = SseParser::new();
        let r = p.feed(&data_line(r#"{"done":true}"#));
        assert!(r.is_empty());
        assert!(p.done());
    }

    #[test]
    fn f8_2_4_consecutive_chunks() {
        let mut p = SseParser::new();
        let mut all = Vec::new();
        all.extend(p.feed(&data_line(r#"{"delta":{"content":"hello"}}"#)));
        all.extend(p.feed(&data_line(r#"{"delta":{"content":" world"}}"#)));
        assert_eq!(all, vec!["hello", " world"]);
    }

    #[test]
    fn f8_2_5_split_utf8_codepoint() {
        let mut full: Vec<u8> = b"data: {\"delta\":{\"content\":\"abc".to_vec();
        full.extend_from_slice("\u{00e9}".as_bytes());
        full.extend_from_slice(b"\"}}\n\n");

        let split_pos = full
            .iter()
            .position(|&b| b == 0xC3)
            .map(|p| p + 1)
            .unwrap_or(0);

        let mut p = SseParser::new();
        let mut results = Vec::new();
        results.extend(p.feed(&full[..split_pos]));
        results.extend(p.feed(&full[split_pos..]));
        assert_eq!(results, vec!["abc\u{00e9}"]);
    }

    #[test]
    fn f8_2_6_split_line_across_chunks() {
        let full = b"data: {\"delta\":{\"content\":\"hello\"}}\n\n";
        let mid = full.len() / 2;

        let mut p = SseParser::new();
        let mut results = Vec::new();
        results.extend(p.feed(&full[..mid]));
        assert!(results.is_empty());
        results.extend(p.feed(&full[mid..]));
        assert_eq!(results, vec!["hello"]);
    }

    #[test]
    fn f8_2_7_ignore_non_data_lines() {
        let mut p = SseParser::new();
        let mut all = Vec::new();
        all.extend(p.feed(b": heartbeat\n"));
        all.extend(p.feed(b"event: foo\n"));
        all.extend(p.feed(&data_line(r#"{"delta":{"content":"yes"}}"#)));
        assert_eq!(all, vec!["yes"]);
    }

    #[test]
    fn f8_2_8_empty_data_line() {
        let mut p = SseParser::new();
        let r = p.feed(b"data:\n\ndata: {\"delta\":{\"content\":\"x\"}}\n\n");
        assert_eq!(r, vec!["x"]);
    }

    #[test]
    fn f8_2_9_data_without_space() {
        let mut p = SseParser::new();
        let r = p.feed(b"data:{\"delta\":{\"content\":\"nospace\"}}\n\n");
        assert_eq!(r, vec!["nospace"]);
    }

    #[test]
    fn f8_2_10_malformed_json() {
        let mut p = SseParser::new();
        let r = p.feed(b"data: {garbage\n\n");
        assert!(r.is_empty());
        assert!(!p.done());
    }

    #[test]
    fn f8_2_11_crlf() {
        let mut p = SseParser::new();
        let r = p.feed(b"data: {\"delta\":{\"content\":\"crlf\"}}\r\n\r\n");
        assert_eq!(r, vec!["crlf"]);
    }

    #[test]
    fn f8_2_12_lf_only() {
        let mut p = SseParser::new();
        let r = p.feed(b"data: {\"delta\":{\"content\":\"lf\"}}\n\n");
        assert_eq!(r, vec!["lf"]);
    }

    #[test]
    fn f8_2_13_cr_only() {
        let mut p = SseParser::new();
        let r = p.feed(b"data: {\"delta\":{\"content\":\"cr\"}}\r\r");
        assert_eq!(r, vec!["cr"]);
    }

    #[test]
    fn f8_2_14_delta_without_content() {
        let mut p = SseParser::new();
        let r = p.feed(&data_line(r#"{"delta":{}}"#));
        assert!(r.is_empty());
    }

    #[test]
    fn f8_2_15_non_string_content() {
        let mut p = SseParser::new();
        let r = p.feed(&data_line(r#"{"delta":{"content":42}}"#));
        assert!(r.is_empty());
    }

    #[test]
    fn f8_2_16_non_dict_json() {
        let mut p = SseParser::new();
        let r = p.feed(b"data: [\"not\",\"dict\"]\n\n");
        assert!(r.is_empty());
    }

    #[test]
    fn f8_2_17_finalize_flushes_remaining() {
        let mut p = SseParser::new();
        p.feed(b"data: {\"delta\":{\"content\":\"partial\"}}");
        let r = p.finalize();
        assert_eq!(r, vec!["partial"]);
    }

    #[test]
    fn done_stops_further_feeds() {
        let mut p = SseParser::new();
        let mut all = Vec::new();
        all.extend(p.feed(&data_line(r#"{"delta":{"content":"before"}}"#)));
        all.extend(p.feed(&data_line(r#"{"done":true}"#)));
        all.extend(p.feed(&data_line(r#"{"delta":{"content":"after"}}"#)));
        assert_eq!(all, vec!["before"]);
        assert!(p.done());
    }

    #[test]
    fn finalize_after_done_yields_nothing() {
        let mut p = SseParser::new();
        p.feed(&data_line(r#"{"done":true}"#));
        let r = p.finalize();
        assert!(r.is_empty());
    }

    #[test]
    fn empty_chunk_returns_empty() {
        let mut p = SseParser::new();
        let r = p.feed(b"");
        assert!(r.is_empty());
    }

    #[test]
    fn default_impl() {
        let p = SseParser::default();
        assert!(!p.done());
    }

    #[test]
    fn multiple_data_lines_in_one_chunk() {
        let mut p = SseParser::new();
        let input = b"data: {\"delta\":{\"content\":\"a\"}}\n\
                      data: {\"delta\":{\"content\":\"b\"}}\n\n";
        let r = p.feed(input);
        assert_eq!(r, vec!["a", "b"]);
    }
}
