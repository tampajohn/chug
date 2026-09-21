//! SSE line parsing and reconnect/retry backoff schedules for the MCP
//! streamable-HTTP transport (SPEC-9). Wired up in round 2 — until then the
//! items here are exercised only by unit tests.
#![allow(dead_code, clippy::empty_line_after_doc_comments)]

use std::sync::Arc;
use std::time::Duration;

/// SSE event emitted by the parser.
#[derive(Debug, PartialEq, Eq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
    pub id: Option<String>,
}

/// Incremental SSE line parser.
pub struct SseParser {
    event: Option<String>,
    id: Option<String>,
    data: String,
    has_data: bool,
    buffer: String,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            event: None,
            id: None,
            data: String::new(),
            has_data: false,
            buffer: String::new(),
        }
    }

    /// Feed a single line (without trailing newline). Returns an event when a blank line is seen.
    pub fn feed_line(&mut self, raw: &str) -> Option<SseEvent> {
        // Trim trailing CR
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.is_empty() {
            // blank line dispatches only if we saw data
            if !self.has_data {
                return None;
            }
            let ev = SseEvent {
                event: self.event.take(),
                data: std::mem::take(&mut self.data),
                id: self.id.take(),
            };
            self.has_data = false;
            self.data.clear();
            return Some(ev);
        }

        if line.starts_with(':') {
            // comment, ignore
            return None;
        }

        if let Some(rest) = line.strip_prefix("event:") {
            self.event = Some(rest.trim_start().to_string());
        } else if let Some(rest) = line.strip_prefix("data:") {
            let val = rest.trim_start().to_string();
            if !self.has_data {
                self.data = val;
                self.has_data = true;
            } else {
                self.data.push('\n');
                self.data.push_str(&val);
            }
        } else if let Some(rest) = line.strip_prefix("id:") {
            self.id = Some(rest.trim_start().to_string());
        }
        None
    }

    /// Convenience: feed a whole chunk split on lines.
    pub fn feed(&mut self, chunk: &str) -> Vec<SseEvent> {
        let mut out = Vec::new();
        // prepend leftover from previous feed
        let mut data = std::mem::take(&mut self.buffer);
        data.push_str(chunk);
        let mut start = 0;
        for (i, b) in data.bytes().enumerate() {
            if b == b'\n' {
                let line = &data[start..i];
                if let Some(ev) = self.feed_line(line) {
                    out.push(ev);
                }
                start = i + 1;
            }
        }
        // keep remainder for next call
        self.buffer = data[start..].to_string();
        out
    }
}

impl Default for SseParser {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_multi_line_data_and_comment() {
        let mut p = SseParser::new();
        assert_eq!(p.feed_line("data: hello"), None);
        assert_eq!(p.feed_line("data: world"), None);
        assert_eq!(p.feed_line(": comment"), None);
        let ev = p.feed_line("");
        assert_eq!(ev, Some(SseEvent { event: None, data: "hello\nworld".to_string(), id: None }));
    }

    #[test]
    fn parser_multi_line_data_empty_first() {
        let mut p = SseParser::new();
        assert_eq!(p.feed_line("data:"), None);
        assert_eq!(p.feed_line("data: x"), None);
        let ev = p.feed_line("").unwrap();
        assert_eq!(ev.data, "\nx");
    }

    #[test]
    fn parser_event_id_and_crlf() {
        let mut p = SseParser::new();
        assert_eq!(p.feed_line("event: msg"), None);
        assert_eq!(p.feed_line("id: 42"), None);
        assert_eq!(p.feed_line("data: foo"), None);
        // CRLF line
        assert_eq!(p.feed_line("data: bar\r"), None);
        let ev = p.feed_line("");
        let e = ev.unwrap();
        assert_eq!(e.event, Some("msg".to_string()));
        assert_eq!(e.id, Some("42".to_string()));
        assert_eq!(e.data, "foo\nbar");
    }

    #[test]
    fn parser_blank_line_dispatches() {
        let mut p = SseParser::new();
        assert!(p.feed_line("data: a").is_none());
        let ev = p.feed_line("").expect("blank line must dispatch pending event");
        assert_eq!(ev.data, "a");
        assert_eq!(ev.event, None);
        assert_eq!(ev.id, None);
        // state resets after dispatch: second event parsed independently
        assert!(p.feed_line("data: b").is_none());
        let ev = p.feed_line("").expect("second blank line must dispatch");
        assert_eq!(ev.data, "b");
        // a blank line with nothing pending dispatches nothing
        assert!(p.feed_line("").is_none());
    }

    #[test]
    fn parser_id_only_no_dispatch() {
        let mut p = SseParser::new();
        assert!(p.feed_line("id: 1").is_none());
        assert!(p.feed_line("event: e").is_none());
        assert!(p.feed_line("").is_none());
    }

    #[test]
    fn parser_feed_chunk_split() {
        let mut p = SseParser::new();
        let evs = p.feed("data: a\n");
        assert!(evs.is_empty());
        let evs = p.feed("data: b\n\n");
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].data, "a\nb");
    }

    #[test]
    fn parser_feed_split_across_chunks() {
        let mut p = SseParser::new();
        let evs = p.feed("data: hel");
        assert!(evs.is_empty());
        let evs = p.feed("lo\ndata: world\n\n");
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].data, "hello\nworld");
    }
}

/// Backoff helpers for SSE reconnect and POST retries.

/// SSE reconnect backoff schedule with injectable clock/jitter.
pub struct SseReconnectBackoff {
    current: Duration,
    base: Duration,
    max: Duration,
}

impl SseReconnectBackoff {
    pub fn new() -> Self {
        Self { current: Duration::from_secs(1), base: Duration::from_secs(1), max: Duration::from_secs(30) }
    }
    pub fn next(&mut self, jitter_factor: f64) -> Duration {
        // apply jitter to base? Spec: starts 1s, doubles, capped at 30s, jittered.
        // We'll jitter the current base before returning and then update current for next.
        let jittered = self.current.as_secs_f64() * jitter_factor;
        let jittered = jittered.max(self.base.as_secs_f64() / 2.0);
        let dur = Duration::from_secs_f64(jittered.min(self.max.as_secs_f64()));
        // advance for next
        let next = self.current * 2;
        self.current = if next > self.max { self.max } else { next };
        dur
    }
}

impl Default for SseReconnectBackoff {
    fn default() -> Self { Self::new() }
}

/// POST connection retry schedule (exactly 3 retries with delays 1s,2s,4s).
/// The sleep between retries is injectable so tests never really sleep.
pub struct PostRetrySchedule {
    attempts: usize,
    sleeper: Arc<dyn Fn(Duration) + Send + Sync>,
}

impl PostRetrySchedule {
    pub fn new() -> Self {
        Self {
            attempts: 0,
            sleeper: Arc::new(std::thread::sleep),
        }
    }

    /// Schedule whose sleeps run `sleeper` instead of `thread::sleep`.
    pub fn with_sleeper(sleeper: Arc<dyn Fn(Duration) + Send + Sync>) -> Self {
        Self { attempts: 0, sleeper }
    }

    pub fn next_delay(&mut self) -> Option<Duration> {
        self.attempts += 1;
        match self.attempts {
            1 => Some(Duration::from_secs(1)),
            2 => Some(Duration::from_secs(2)),
            3 => Some(Duration::from_secs(4)),
            _ => None,
        }
    }

    /// Sleep for the next retry delay. Returns false (and does not sleep)
    /// once the schedule is exhausted — i.e. the POST has had its 3 retries.
    pub fn sleep_next(&mut self) -> bool {
        match self.next_delay() {
            Some(d) => {
                (self.sleeper)(d);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod backoff_tests {
    use super::*;
    #[test]
    fn sse_backoff_unjittered_shape() {
        let mut b = SseReconnectBackoff::new();
        // first call returns jittered 1s with factor 1.0
        assert_eq!(b.next(1.0), Duration::from_secs(1));
        assert_eq!(b.next(1.0), Duration::from_secs(2));
        assert_eq!(b.next(1.0), Duration::from_secs(4));
        assert_eq!(b.next(1.0), Duration::from_secs(8));
        assert_eq!(b.next(1.0), Duration::from_secs(16));
        assert_eq!(b.next(1.0), Duration::from_secs(30)); // capped
        assert_eq!(b.next(1.0), Duration::from_secs(30));
    }

    #[test]
    fn post_retry_schedule_exact() {
        let mut s = PostRetrySchedule::new();
        assert_eq!(s.next_delay(), Some(Duration::from_secs(1)));
        assert_eq!(s.next_delay(), Some(Duration::from_secs(2)));
        assert_eq!(s.next_delay(), Some(Duration::from_secs(4)));
        assert_eq!(s.next_delay(), None);
    }

    /// The injected sleeper receives exactly the 1s/2s/4s schedule and the
    /// schedule then reports exhaustion — tests use this to never really sleep.
    #[test]
    fn post_retry_injected_sleeper() {
        use std::sync::Mutex;
        let slept = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&slept);
        let mut s = PostRetrySchedule::with_sleeper(Arc::new(move |d| {
            record.lock().unwrap().push(d);
        }));
        assert!(s.sleep_next());
        assert!(s.sleep_next());
        assert!(s.sleep_next());
        assert!(!s.sleep_next());
        assert_eq!(
            slept.lock().unwrap().as_slice(),
            &[
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(4)
            ]
        );
    }

    #[test]
    fn jitter_bounds() {
        let mut b = SseReconnectBackoff::new();
        // jitter factor 0.5 should stay >= base/2 = 0.5s
        let d = b.next(0.5);
        assert!(d >= Duration::from_millis(500));
        // reset
        let mut b2 = SseReconnectBackoff::new();
        let d2 = b2.next(1.0);
        assert_eq!(d2, Duration::from_secs(1));
    }
}
