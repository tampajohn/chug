//! SSE line parsing and reconnect/retry backoff schedules for the MCP
//! streamable-HTTP transport (SPEC-9). Wired up in round 2 — until then the
//! items here are exercised only by unit tests.
#![allow(dead_code)]

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
    seen_empty: bool,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            event: None,
            id: None,
            data: String::new(),
            seen_empty: false,
        }
    }

    /// Feed a single line (without trailing newline). Returns an event when a blank line is seen.
    pub fn feed_line(&mut self, raw: &str) -> Option<SseEvent> {
        // Trim trailing CR
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.is_empty() {
            // blank line dispatches
            if self.data.is_empty() && self.event.is_none() && self.id.is_none() {
                return None;
            }
            let ev = SseEvent {
                event: self.event.take(),
                data: std::mem::take(&mut self.data),
                id: self.id.take(),
            };
            self.seen_empty = false;
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
            if self.data.is_empty() {
                self.data = val;
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
        for line in chunk.split_inclusive('\n') {
            let line = line.trim_end_matches(&['\n', '\r'][..]);
            if let Some(ev) = self.feed_line(line) {
                out.push(ev);
            }
        }
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
}

/// Backoff helpers for SSE reconnect and POST retries.
pub struct Backoff {
    base: Duration,
    max: Duration,
}

impl Backoff {
    pub fn new(base: Duration, max: Duration) -> Self {
        Self { base, max }
    }

    /// Next delay with jitter injectable. jitter_fn returns a factor in (0,1] used as multiplier? Spec: jitter stays within [base/2, base] or similar.
    /// We'll implement classic jitter: delay = base * jitter_factor where jitter_factor in [0.5, 1.0]
    pub fn next(&self, current: Duration, jitter: f64) -> Duration {
        let jittered = (current.as_secs_f64() * jitter).max(self.base.as_secs_f64() / 2.0);
        // cap not applied here
        Duration::from_secs_f64(jittered)
    }
}

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

/// POST connection retry schedule (exactly 3 attempts with delays 1s,2s,4s)
pub struct PostRetrySchedule {
    attempts: usize,
}

impl PostRetrySchedule {
    pub fn new() -> Self { Self { attempts: 0 } }
    pub fn next_delay(&mut self) -> Option<Duration> {
        self.attempts += 1;
        match self.attempts {
            1 => Some(Duration::from_secs(1)),
            2 => Some(Duration::from_secs(2)),
            3 => Some(Duration::from_secs(4)),
            _ => None,
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
