//! T37 — `web_fetch`: read-only HTTP(S) GET with hard bounds.
//!
//! The loop could already reach the network through `bash` (`curl`), but a raw
//! shell fetch is unbounded and unaudited: no size cap, no content-type gate,
//! no text extraction. `web_fetch` is the bounded, token-safe surface on top
//! of the same capability:
//!
//! - **Transport**: GET only; `http://`/`https://` only (any other scheme is a
//!   tool error naming the scheme); redirects followed up to
//!   [`WEB_FETCH_MAX_REDIRECTS`] (the 6th is refused); connect 10s, total 30s.
//! - **Content**: `text/html` is tag-stripped to visible text (hand-rolled
//!   state machine — no new dependency); other `text/*` and
//!   `application/json` return the body verbatim; any other content type is
//!   refused by name (binary refusal). A missing Content-Type is text.
//! - **Bounds**: the output is char-capped at `max_chars` (default
//!   [`WEB_FETCH_DEFAULT_MAX_CHARS`], hard ceiling
//!   [`WEB_FETCH_MAX_CHARS_CEILING`] — higher requests are clamped, not
//!   rejected) with a `[truncated at N chars]` marker; the wire side is
//!   byte-capped so a multi-gigabyte response can never balloon memory.
//! - **Errors, never aborts**: non-2xx statuses (with a ≤500-char body
//!   preview), DNS/connect/timeout/redirect-overflow all come back as tool
//!   errors. One attempt, no retry loop — the model can re-call.
//!
//! Sandbox honesty: `web_fetch` reaches OUTSIDE the cwd sandbox by design (it
//! is network, not filesystem) — the same candor the `delegate` schema uses.
//!
//! Wall-clock safety (T6 discipline): the fetch runs on a worker thread and
//! the caller bounds it with a channel `recv_timeout`, in the spirit of
//! `mcp_http`'s request pattern — a wedged peer costs the tool call its 30s
//! budget, never the driver loop.

use std::io::Read;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use serde_json::{json, Value};

use crate::build_info::VERSION;
use crate::tools::ToolResult;

/// Default output cap in characters — the token-safe size of one fetch.
/// Pinned by test (non-vacuousness leg): a body just over this size must come
/// back truncated with the marker, so removing the cap fails the suite.
pub(crate) const WEB_FETCH_DEFAULT_MAX_CHARS: usize = 20_000;
/// Hard ceiling on `max_chars`. A larger request is clamped down, never an
/// error (spec req 1/2).
pub(crate) const WEB_FETCH_MAX_CHARS_CEILING: usize = 100_000;
/// Redirect cap: 5 hops are followed, the 6th is a tool error naming
/// redirects (spec req 2).
pub(crate) const WEB_FETCH_MAX_REDIRECTS: usize = 5;
/// Connect-phase timeout (spec req 2; mirrors `mcp_http`'s CONNECT_TIMEOUT).
const WEB_FETCH_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Total budget of one `web_fetch` call, enforced caller-side with a channel
/// `recv_timeout` (spec req 2: "total 30s"). The worker's reqwest client also
/// carries this as its per-request timeout, but the caller-side deadline is
/// what makes the total bound true for multi-redirect chains and trickling
/// bodies.
const WEB_FETCH_TOTAL_TIMEOUT: Duration = Duration::from_secs(30);
/// Wire-side body cap: at most this many bytes (+1 overflow probe) are pulled
/// off the network per fetch. Generous enough that any body that could yield
/// [`WEB_FETCH_MAX_CHARS_CEILING`] chars of text arrives complete (HTML
/// stripping only shrinks), tight enough that memory stays bounded.
const WEB_FETCH_BODY_BYTE_CAP: usize = 2 * 1024 * 1024;
/// Non-2xx body preview length (spec req 4: ≤500 chars).
const NON_2XX_PREVIEW_CHARS: usize = 500;
/// Bytes read to build that preview (500 chars of UTF-8 fit comfortably).
const NON_2XX_PREVIEW_BYTE_CAP: usize = 2_048;
/// Longest `&entity;` name scanned before giving up and emitting the `&`.
const MAX_ENTITY_LEN: usize = 32;

/// JSON schema for the `web_fetch` tool, registered alongside the builtins.
pub fn schema() -> Value {
    json!({
        "name": "web_fetch",
        "description": "Fetch a URL over HTTP(S) and return its text. GET only, http:// or https:// only (any other scheme is refused); follows up to 5 redirects; connect 10s, total 30s. Output is size-capped at max_chars (default 20000; a request above the 100000 hard ceiling is clamped, not rejected); text/html is tag-stripped to visible text, other text/* and application/json return the body verbatim, and any other content type (binaries) is refused. Unlike the filesystem tools this reaches OUTSIDE the cwd sandbox by design — it is network, not filesystem. Non-2xx statuses and transport failures return as tool errors, never aborts; one attempt, no retry.",
        "input_schema": {
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "Absolute http:// or https:// URL to GET"},
                "max_chars": {"type": "integer", "description": "Maximum characters of processed text to return (default 20000; above the 100000 ceiling it is clamped down)"}
            },
            "required": ["url"]
        }
    })
}

/// Dispatch entry for the `web_fetch` arm in `tools::inner`.
pub fn web_fetch(input: &Value) -> anyhow::Result<ToolResult> {
    let url = input
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing or non-string field: url"))?;
    let max_chars = max_chars_from(input)?;
    let content = fetch_text(url, max_chars)?;
    Ok(ToolResult {
        content,
        is_error: false,
    })
}

/// Resolve `max_chars` from the tool input: default
/// [`WEB_FETCH_DEFAULT_MAX_CHARS`] when absent, clamped to
/// `[1, WEB_FETCH_MAX_CHARS_CEILING]` when present (a too-large request is a
/// clamp, not an error — spec req 1/2), a tool error when not an integer.
fn max_chars_from(input: &Value) -> anyhow::Result<usize> {
    match input.get("max_chars") {
        None | Some(Value::Null) => Ok(WEB_FETCH_DEFAULT_MAX_CHARS),
        Some(v) => match v.as_u64() {
            Some(n) => Ok((n as usize).clamp(1, WEB_FETCH_MAX_CHARS_CEILING)),
            None => bail!("max_chars must be a non-negative integer (a char cap)"),
        },
    }
}

/// GET one URL and return the processed text, char-capped at `max_chars`.
///
/// The caller-side deadline ([`WEB_FETCH_TOTAL_TIMEOUT`]) is the hard total
/// bound: the fetch itself runs on a detached worker thread, so a peer that
/// tricks its way past every client-side timeout still costs this call only
/// its 30s — then the tool returns an error and the loop moves on.
pub(crate) fn fetch_text(url: &str, max_chars: usize) -> anyhow::Result<String> {
    // Scheme gate first: `file:///etc/passwd` and `ftp://…` never reach the
    // network at all, and the error names the offending scheme (spec req 2,
    // test leg 6).
    let parsed = reqwest::Url::parse(url).map_err(|e| anyhow!("invalid URL {url:?}: {e}"))?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        bail!("unsupported URL scheme {scheme:?} — web_fetch is GET over http:// or https:// only");
    }

    let (tx, rx) = mpsc::channel();
    let url = parsed.to_string();
    // Detached on purpose: the `recv_timeout` below is the bound, so joining
    // here would double the worst case instead of capping it.
    let _ = thread::Builder::new()
        .name("web_fetch".into())
        .spawn(move || {
            let _ = tx.send(fetch_worker(url, max_chars));
        })
        .context("spawning web_fetch worker")?;

    match rx.recv_timeout(WEB_FETCH_TOTAL_TIMEOUT) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => bail!(
            "web_fetch timed out after {}s (connect 10s, total 30s)",
            WEB_FETCH_TOTAL_TIMEOUT.as_secs()
        ),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            bail!("web_fetch worker exited without a result (internal panic)")
        }
    }
}

/// The fetch itself: one GET attempt, classify the response, cap the text.
fn fetch_worker(url: String, max_chars: usize) -> anyhow::Result<String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(user_agent())
        .connect_timeout(WEB_FETCH_CONNECT_TIMEOUT)
        .timeout(WEB_FETCH_TOTAL_TIMEOUT)
        .redirect(redirect_policy())
        .build()
        .context("building web_fetch http client")?;

    let resp = client.get(&url).send().map_err(|e| classify_transport(&e))?;

    // Non-2xx → tool error naming the status, with a ≤500-char body preview
    // (spec req 4). One attempt: no retry loop.
    let status = resp.status();
    if !status.is_success() {
        let mut limited = resp.take(NON_2XX_PREVIEW_BYTE_CAP as u64);
        let mut bytes = Vec::new();
        let _ = limited.read_to_end(&mut bytes);
        let preview: String = String::from_utf8_lossy(&bytes)
            .chars()
            .take(NON_2XX_PREVIEW_CHARS)
            .collect();
        bail!("HTTP {status}: non-2xx response from GET {url}; body preview: {preview}");
    }

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    let kind = classify_content_type(content_type)?;

    let (raw, body_truncated) = read_body_capped(resp)?;

    let mut text = match kind {
        ContentKind::Html => html_to_text(&raw),
        ContentKind::Text => raw,
    };

    if text.chars().count() > max_chars {
        text = text.chars().take(max_chars).collect();
        text.push_str(&format!("\n[truncated at {max_chars} chars]"));
    } else if body_truncated {
        // The wire cap cut the body but the text still fits the char cap
        // (heavy markup stripped away): say so rather than return silently
        // partial prose.
        text.push_str(&format!(
            "\n[response body cut at the {}-byte read cap; text may be incomplete]",
            WEB_FETCH_BODY_BYTE_CAP
        ));
    }
    Ok(text)
}

/// The one fixed request header (spec req 7: nothing else, no cookies/auth).
fn user_agent() -> String {
    format!("chug/{VERSION}")
}

/// Redirect policy: follow up to [`WEB_FETCH_MAX_REDIRECTS`], refuse the next
/// one with a message naming redirects (spec req 2: cap 5, 6th → tool error).
///
/// reqwest hands the policy `previous()` = every URL already requested in the
/// chain, the initial URL first — so the number of redirects *followed* is
/// `len() - 1`.
fn redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        let followed = attempt.previous().len().saturating_sub(1);
        if followed >= WEB_FETCH_MAX_REDIRECTS {
            attempt.error(format!(
                "redirect cap of {WEB_FETCH_MAX_REDIRECTS} exceeded — refusing to follow redirect #{}",
                followed + 1
            ))
        } else {
            attempt.follow()
        }
    })
}

/// Map a reqwest transport error onto a tool error naming its class (spec req
/// 4: DNS/connect/timeout/redirect-overflow). Never a panic, never a retry.
fn classify_transport(e: &reqwest::Error) -> anyhow::Error {
    if e.is_redirect() {
        return anyhow!("too many redirects: {}", root_message(e));
    }
    if e.is_builder() && root_message(e).contains("scheme is not allowed") {
        return anyhow!("unsupported redirect scheme — only http:// and https:// are followed");
    }
    if e.is_connect() {
        return anyhow!("connection failed (DNS or connect): {}", root_message(e));
    }
    if e.is_timeout() {
        return anyhow!("request timed out (connect 10s, total 30s): {}", root_message(e));
    }
    anyhow!("request failed: {}", root_message(e))
}

/// Innermost message of an error chain: reqwest wraps the interesting part
/// (our redirect message, the io error, the hyper cause) one or two levels
/// down, and the model wants that, not "error following redirect".
fn root_message(e: &(dyn std::error::Error + 'static)) -> String {
    let mut msg = e.to_string();
    let mut cur = e.source();
    while let Some(cause) = cur {
        msg = cause.to_string();
        cur = cause.source();
    }
    msg
}

/// What to do with the body, decided from the Content-Type header (spec req
/// 3). A missing (or parameter-only) header is treated as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContentKind {
    /// `text/html` — strip to visible text.
    Html,
    /// Other `text/*`, `application/json`, or no header — body verbatim.
    Text,
}

fn classify_content_type(header: Option<&str>) -> anyhow::Result<ContentKind> {
    let media = match header.map(str::trim) {
        None | Some("") => return Ok(ContentKind::Text),
        Some(v) => v.split(';').next().unwrap_or("").trim().to_ascii_lowercase(),
    };
    match media.as_str() {
        "text/html" => Ok(ContentKind::Html),
        m if m.starts_with("text/") => Ok(ContentKind::Text),
        "application/json" => Ok(ContentKind::Text),
        other => bail!(
            "refused content type {other:?} — web_fetch returns text only \
             (html is tag-stripped); binary content is not fetched"
        ),
    }
}

/// Read the response body under the wire cap: at most
/// [`WEB_FETCH_BODY_BYTE_CAP`] (+1 overflow probe) bytes leave the socket, so
/// no response can balloon memory. Returns the lossy-UTF-8 text and whether
/// the cap cut the body short.
fn read_body_capped(resp: reqwest::blocking::Response) -> anyhow::Result<(String, bool)> {
    let mut limited = resp.take(WEB_FETCH_BODY_BYTE_CAP as u64 + 1);
    let mut bytes = Vec::new();
    limited
        .read_to_end(&mut bytes)
        .map_err(|e| anyhow!("reading response body failed: {e}"))?;
    let truncated = bytes.len() > WEB_FETCH_BODY_BYTE_CAP;
    // Cut on a char boundary so the lossy decode never fabricates a
    // replacement char from our own truncation.
    let mut len = bytes.len().min(WEB_FETCH_BODY_BYTE_CAP);
    // Only a real cut can split a sequence: when the body fit under the cap
    // (len == bytes.len()) there is nothing to repair — and indexing `bytes`
    // at `len` panics. Otherwise step the cut left while the byte just past
    // it is a UTF-8 continuation byte, so the kept text ends on a whole char.
    while len > 0 && len < bytes.len() && bytes[len] & 0xC0 == 0x80 {
        // Continuation byte: step back to the start of the UTF-8 sequence.
        len -= 1;
    }
    bytes.truncate(len);
    Ok((String::from_utf8_lossy(&bytes).into_owned(), truncated))
}

/// Strip an HTML document down to visible text (spec req 3): drop
/// `<!-- comments -->`, drop `<script>`/`<style>` blocks whole (their content
/// is code, not prose), drop every remaining tag, decode the handful of
/// entities that shows up in prose, then collapse whitespace runs (including
/// the boundaries of block-level tags) to single spaces.
///
/// Hand-rolled byte-level state machine — the spec forbids adding a
/// heavyweight HTML crate, and none is present in Cargo.toml's tree. All
/// scanning is on bytes (tag delimiters are ASCII, and UTF-8 continuation
/// bytes never collide with ASCII), so no slice can land mid-character.
pub(crate) fn html_to_text(html: &str) -> String {
    let b = html.as_bytes();
    let mut out = String::with_capacity(html.len() / 2 + 16);
    // A whitespace run is pending: flushed as one ' ' only when real text
    // follows, which collapses runs; the final output is trimmed (leading
    // and trailing whitespace is not "visible text").
    let mut pending_space = false;
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            b'<' => {
                if b[i..].starts_with(b"<!--") {
                    i = match find_sub(b, i + 4, b"-->") {
                        Some(j) => j + 3,
                        None => b.len(),
                    };
                    continue;
                }
                if b[i..].starts_with(b"<!") || b[i..].starts_with(b"<?") {
                    // `<!DOCTYPE …>`, other `<!…>` declarations, and `<?…?>`
                    // processing instructions are markup, not prose — skip to
                    // the closing `>` (quote-aware, same as any tag).
                    i = skip_tag(b, i).unwrap_or(b.len());
                    continue;
                }
                let (_, name) = tag_name_at(b, i);
                match name {
                    Some(name) => {
                        let lower = name.to_ascii_lowercase();
                        let block = is_block_tag(&lower);
                        i = if lower == "script" || lower == "style" {
                            skip_raw_block(b, i, lower.as_bytes())
                        } else {
                            skip_tag(b, i).unwrap_or(b.len())
                        };
                        if block {
                            pending_space = true;
                        }
                    }
                    None => {
                        // A bare `<` that opens nothing tag-like ("i <3 it")
                        // is text, not markup.
                        if pending_space {
                            out.push(' ');
                            pending_space = false;
                        }
                        out.push('<');
                        i += 1;
                    }
                }
            }
            b'&' => match decode_entity_at(b, i) {
                Some((decoded, next)) => {
                    if pending_space {
                        out.push(' ');
                        pending_space = false;
                    }
                    out.push_str(&decoded);
                    i = next;
                }
                None => {
                    if pending_space {
                        out.push(' ');
                        pending_space = false;
                    }
                    out.push('&');
                    i += 1;
                }
            },
            c if c.is_ascii_whitespace() => {
                pending_space = true;
                i += 1;
            }
            _ => {
                let ch = html[i..]
                    .chars()
                    .next()
                    .expect("byte index is at a char boundary");
                if pending_space {
                    out.push(' ');
                    pending_space = false;
                }
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    out.trim().to_string()
}

/// Index of `needle` in `haystack[from..]`, if present.
fn find_sub(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from > haystack.len() || needle.is_empty() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// Parse the tag name starting at `b[i] == b'<'`. Returns (index just past the
/// name, `Some(name)`), or `(i + 1, None)` when no plausible name follows.
/// Names: optional `/`, then `[A-Za-z][A-Za-z0-9:_-]*`.
fn tag_name_at(b: &[u8], i: usize) -> (usize, Option<String>) {
    let mut j = i + 1;
    if j < b.len() && b[j] == b'/' {
        j += 1;
    }
    let start = j;
    while j < b.len() && (b[j].is_ascii_alphanumeric() || matches!(b[j], b':' | b'-' | b'_')) {
        j += 1;
    }
    if j == start || !b[start].is_ascii_alphabetic() {
        return (i + 1, None);
    }
    (j, Some(String::from_utf8_lossy(&b[start..j]).into_owned()))
}

/// Skip a `<tag …>` opener, returning the index just past its `>`.
/// Quote-aware: a `>` inside a quoted attribute value does not close the tag.
fn skip_tag(b: &[u8], lt: usize) -> Option<usize> {
    let mut i = lt + 1;
    let mut quote: Option<u8> = None;
    while i < b.len() {
        match (quote, b[i]) {
            (Some(q), c) if c == q => quote = None,
            (None, b'"') | (None, b'\'') => quote = Some(b[i]),
            (None, b'>') => return Some(i + 1),
            _ => {}
        }
        i += 1;
    }
    None
}

/// Skip a whole `<script …>…</script>` / `<style …>…</style>` block: from the
/// open tag at `lt` through the matching case-insensitive closer. An
/// unterminated block runs to EOF (everything after it is code anyway).
fn skip_raw_block(b: &[u8], lt: usize, name: &[u8]) -> usize {
    let mut i = lt + 1;
    while i < b.len() {
        match b[i..].iter().position(|&c| c == b'<') {
            Some(off) => {
                let cand = i + off;
                if b.get(cand + 1) == Some(&b'/')
                    && b[cand + 2..].len() >= name.len()
                    && b[cand + 2..cand + 2 + name.len()].eq_ignore_ascii_case(name)
                {
                    return skip_tag(b, cand).unwrap_or(b.len());
                }
                i = cand + 1;
            }
            None => return b.len(),
        }
    }
    b.len()
}

/// Tags whose presence should break the text flow: each gets a whitespace
/// boundary in the extracted text, so `<p>a</p><p>b</p>` reads `a b`, not
/// `ab`. Inline tags (`<a>`, `<b>`, `<span>`…) contribute nothing.
fn is_block_tag(name: &str) -> bool {
    matches!(
        name,
        "address" | "article" | "aside" | "blockquote" | "br" | "caption" | "dd" | "details"
            | "div" | "dl" | "dt" | "fieldset" | "figcaption" | "figure" | "footer" | "form"
            | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "head" | "header" | "hr" | "html"
            | "li" | "main" | "nav" | "ol" | "p" | "pre" | "section" | "summary" | "table"
            | "tbody" | "td" | "tfoot" | "th" | "thead" | "title" | "tr" | "ul"
    )
}

/// Try to decode one HTML entity starting at `b[amp] == b'&'`. Returns the
/// decoded text and the index just past the `;`, or `None` when the bytes do
/// not form a recognized entity (the caller emits the `&` verbatim).
fn decode_entity_at(b: &[u8], amp: usize) -> Option<(String, usize)> {
    let mut j = amp + 1;
    while j < b.len() && j - amp <= MAX_ENTITY_LEN {
        match b[j] {
            b';' => {
                let name = std::str::from_utf8(&b[amp + 1..j]).ok()?;
                return decode_entity_name(name).map(|s| (s, j + 1));
            }
            // Entity refs are ASCII alphanumerics (plus `#` for numeric);
            // anything else ends the scan, so prose like "R&D - ok" or
            // "a & b < c" survives untouched.
            c if c.is_ascii_alphanumeric() || c == b'#' => j += 1,
            _ => return None,
        }
    }
    None
}

fn decode_entity_name(name: &str) -> Option<String> {
    let named = match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{a0}'),
        _ => None,
    };
    match named {
        Some(c) => Some(c.to_string()),
        None => {
            let digits = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X"));
            match digits {
                Some(d) => codepoint(d, 16),
                None => name.strip_prefix('#').and_then(|d| codepoint(d, 10)),
            }
        }
    }
}

fn codepoint(digits: &str, radix: u32) -> Option<String> {
    if digits.is_empty() {
        return None;
    }
    u32::from_str_radix(digits, radix)
        .ok()
        .and_then(char::from_u32)
        .map(|c| c.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp_http::tests::{
        accept_conn, assert_dead_port, dead_port, read_request, write_response_close, Observed,
    };
    use crate::tools::{dispatch, ToolCtx};
    use std::net::TcpListener;
    use std::time::Instant;
    use tempfile::TempDir;

    // ---------- stub harness (T6 discipline: bounded accept, socket timeouts) ----------
    //
    // The request/response half is reused from mcp_http's T6 harness
    // (`accept_conn` carries a 10s accept deadline and sets read+write
    // timeouts on every accepted socket; `read_request` bounds the head and
    // body read) so a wedged client fails a test in seconds, never hangs the
    // suite. T31's `dead_port()` probe is reused as-is for the refused leg.

    struct StubResp {
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    impl StubResp {
        fn text(content_type: Option<&str>, body: &str) -> Self {
            let mut headers = Vec::new();
            if let Some(ct) = content_type {
                headers.push(("content-type".to_string(), ct.to_string()));
            }
            StubResp {
                status: 200,
                headers,
                body: body.as_bytes().to_vec(),
            }
        }

        fn redirect(location: &str) -> Self {
            StubResp {
                status: 302,
                headers: vec![("location".to_string(), location.to_string())],
                body: b"redirecting".to_vec(),
            }
        }
    }

    /// Serve `n` request/response exchanges on 127.0.0.1, one connection per
    /// request (every response carries `connection: close`, so accept order is
    /// deterministic). The stub serves exactly `n`: an unexpected (n+1)th
    /// request is refused, which fails any success assertion loudly instead of
    /// hanging. Returns the base URL and the stub's join handle — join it in
    /// every test so a handler panic fails the test.
    fn serve(
        n: usize,
        handler: impl Fn(&Observed) -> StubResp + Send + 'static,
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            for _ in 0..n {
                let mut stream = accept_conn(&listener);
                let req = read_request(&mut stream);
                let resp = handler(&req);
                let hdrs: Vec<(&str, &str)> = resp
                    .headers
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect();
                write_response_close(&mut stream, resp.status, &hdrs, &resp.body);
            }
        });
        (url, handle)
    }

    /// Request target (`GET /x?q HTTP/1.1` → `/x?q`).
    fn target_of(req: &Observed) -> String {
        req.request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("/")
            .to_string()
    }

    fn tool_ctx(cwd: &std::path::Path) -> ToolCtx {
        ToolCtx {
            cwd: cwd.to_path_buf(),
            bash_timeout: Duration::from_secs(crate::tools::BASH_TIMEOUT_SECS),
        }
    }

    /// Run the tool through the real dispatcher so error wrapping
    /// (`tool error: …`) is part of what is asserted.
    fn fetch_via_dispatch(url: &str, max_chars: Option<u64>) -> crate::tools::ToolResult {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let mut input = json!({ "url": url });
        if let Some(m) = max_chars {
            input["max_chars"] = json!(m);
        }
        dispatch(&ctx, "web_fetch", &input)
    }

    // ---------- leg 1: text/plain verbatim (plus GET + the fixed UA) ----------

    #[test]
    fn plain_text_body_returns_verbatim_with_get_and_chug_user_agent() {
        let body = "hello from the stub\nsecond line\n".to_string();
        let expected_ua = format!("chug/{VERSION}");
        let body_for_stub = body.clone();
        let (url, stub) = serve(1, move |req| {
            assert_eq!(req.request_line.split(' ').next(), Some("GET"), "GET only");
            assert_eq!(
                req.header("user-agent"),
                Some(expected_ua.as_str()),
                "one fixed User-Agent header"
            );
            StubResp::text(Some("text/plain"), &body_for_stub)
        });
        let result = fetch_via_dispatch(&format!("{url}/doc.txt"), None);
        stub.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(result.content, body, "text/plain is verbatim under the cap");
    }

    // ---------- leg 1b: missing Content-Type is treated as text ----------

    #[test]
    fn missing_content_type_is_treated_as_text() {
        let (url, stub) = serve(1, |_req| StubResp::text(None, "bare bytes\nno header\n"));
        let result = fetch_via_dispatch(&format!("{url}/noct"), None);
        stub.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(result.content, "bare bytes\nno header\n");
    }

    // ---------- leg 2: text/html is tag-stripped ----------

    #[test]
    fn html_strips_script_style_and_tags_keeps_visible_text_collapsed() {
        let html = concat!(
            "<!DOCTYPE html>\n<html><head><title>Stub Page</title>\n",
            "<style>p { color: red; font-weight: bold }</style></head>\n",
            "<body>\n  <h1>Visible   Heading</h1>\n",
            "<script>var secret = \"<b>script secret</b>\"; alert(1);</script>\n",
            "<p>First  para with <a href=\"/x\">a   link</a> &amp; 5 &lt; 6.</p>\n",
            "<ul><li>one</li><li>two</li></ul>\n",
            "<p>i <3 markup</p>\n",
            "<!-- a comment that must vanish -->\n",
            "</body>\n</html>\n"
        )
        .to_string();
        let (url, stub) = serve(1, move |_req| StubResp::text(Some("text/html"), &html));
        let result = fetch_via_dispatch(&format!("{url}/page"), None);
        stub.join().unwrap();
        assert!(!result.is_error, "{}", result.content);

        // Script, style, and comment content GONE (spec leg 2) — including
        // the markup-looking string inside the script.
        for gone in [
            "script secret",
            "alert(",
            "var secret",
            "color: red",
            "font-weight",
            "a comment that must vanish",
        ] {
            assert!(
                !result.content.contains(gone),
                "leaked {gone:?}: {:?}",
                result.content
            );
        }
        // Tags gone. (Targeted leaked-TAG asserts, not naive `<`/`>`:
        // the expected visible text itself decodes `&lt;` → "5 < 6" and
        // keeps the bare `<` of "i <3" as text — a real tag leak has a
        // letter, `/`, or `!` right after the `<`.)
        for tag in [
            "<h1", "</", "<p>", "<a ", "<ul", "<li", "<body", "<html", "<head",
            "<title>", "<style", "<script", "<!", "<div", "<span",
        ] {
            assert!(
                !result.content.contains(tag),
                "tag leaked {tag:?}: {:?}",
                result.content
            );
        }
        // Whitespace collapsed.
        for ws in ["  ", "\n", "\t", "\r"] {
            assert!(
                !result.content.contains(ws),
                "whitespace not collapsed ({ws:?}): {:?}",
                result.content
            );
        }
        // The exact extraction: visible text only, block tags as boundaries,
        // entities decoded, single spaces.
        assert_eq!(
            result.content,
            "Stub Page Visible Heading First para with a link & 5 < 6. one two i <3 markup"
        );
    }

    #[test]
    fn html_edge_cases_quote_aware_unterminated_and_entities() {
        // A `>` inside a quoted attribute does not close the tag.
        assert_eq!(html_to_text("<a title=\"a > b\">shown</a>"), "shown");
        // DOCTYPE / processing instructions are dropped, not emitted as text.
        assert_eq!(html_to_text("<!DOCTYPE html><p>doc</p>"), "doc");
        assert_eq!(html_to_text("<?php echo 1; ?><p>doc</p>"), "doc");
        // Unterminated tag swallows the rest (nothing sensible to extract)…
        assert_eq!(html_to_text("<p class=\"x"), "");
        // …but a TERMINATED tag does not swallow the text that follows it.
        assert_eq!(html_to_text("<p>truncated"), "truncated");
        // A bare `<` that opens nothing tag-like is text; an unrecognized
        // entity stays verbatim.
        assert_eq!(html_to_text("i <3 you &ok"), "i <3 you &ok");
        assert_eq!(html_to_text("R&D - ok"), "R&D - ok");
        // Block tags break the flow; inline tags do not.
        assert_eq!(html_to_text("a</p>b"), "a b");
        assert_eq!(html_to_text("a<b>b</b>c"), "abc");
        // Raw blocks are matched case-insensitively.
        assert_eq!(html_to_text("<SCRIPT>x</SCRIPT>ok"), "ok");
        assert_eq!(html_to_text("<STYLE>x</style>ok"), "ok");
        // Entities: named, decimal, hex, nbsp.
        assert_eq!(html_to_text("&#65;&#x42;&nbsp;!"), "AB\u{a0}!");
        assert_eq!(html_to_text(""), "");
        assert_eq!(html_to_text("   "), "");
    }

    // ---------- leg 3: 404 names the status, body preview survives ----------

    #[test]
    fn not_found_names_status_and_keeps_body_preview() {
        let (url, stub) = serve(1, |_req| StubResp {
            status: 404,
            headers: vec![("content-type".to_string(), "text/plain".to_string())],
            body: b"the page you want ran away forever".to_vec(),
        });
        let result = fetch_via_dispatch(&format!("{url}/gone"), None);
        stub.join().unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("404"), "{}", result.content);
        assert!(
            result.content.contains("the page you want ran away forever"),
            "body preview lost: {}",
            result.content
        );
    }

    // ---------- leg 4: redirect chains ----------

    #[test]
    fn two_redirect_hops_land_on_final_body() {
        let (url, stub) = serve(3, |req| match target_of(req).as_str() {
            "/start" => StubResp::redirect("/hop1"),
            "/hop1" => StubResp::redirect("/final"),
            _ => StubResp::text(Some("text/plain"), "arrived after two hops"),
        });
        let result = fetch_via_dispatch(&format!("{url}/start"), None);
        stub.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(result.content, "arrived after two hops");
    }

    #[test]
    fn sixth_redirect_is_refused_naming_redirects() {
        // /r0 → /r1 → … all redirect; the 6th redirect (off /r5) must be
        // refused, so the stub sees exactly 6 requests and never a target
        // past /r5.
        let (url, stub) = serve(6, |req| {
            let target = target_of(req);
            let n: usize = target.trim_start_matches("/r").parse().unwrap();
            assert!(n < 6, "client followed {n} redirects — cap broken");
            StubResp::redirect(&format!("/r{}", n + 1))
        });
        let result = fetch_via_dispatch(&format!("{url}/r0"), None);
        stub.join().unwrap();
        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("redirect"), "{}", result.content);
        assert!(
            result.content.contains("5"),
            "error should name the cap: {}",
            result.content
        );
    }

    // ---------- leg 5: truncation, explicit cap, ceiling clamp ----------

    #[test]
    fn body_over_explicit_max_chars_is_truncated_with_marker() {
        let body = "x".repeat(5_000);
        let (url, stub) = serve(1, move |_req| StubResp::text(Some("text/plain"), &body));
        let result = fetch_via_dispatch(&format!("{url}/big"), Some(1_000));
        stub.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(
            result.content.chars().filter(|c| *c == 'x').count(),
            1_000,
            "explicit small max_chars honored"
        );
        assert!(
            result.content.contains("[truncated at 1000 chars]"),
            "marker missing: {:?}",
            result.content
        );
    }

    #[test]
    fn max_chars_above_hard_ceiling_is_clamped_not_rejected() {
        let body = "y".repeat(120_000);
        let (url, stub) = serve(1, move |_req| StubResp::text(Some("text/plain"), &body));
        // 250_000 requested → clamped to the 100_000 ceiling (no error).
        let result = fetch_via_dispatch(&format!("{url}/clamp"), Some(250_000));
        stub.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(
            result.content.chars().filter(|c| *c == 'y').count(),
            WEB_FETCH_MAX_CHARS_CEILING,
            "clamped to the hard ceiling"
        );
        assert!(
            result
                .content
                .contains(&format!("[truncated at {} chars]", WEB_FETCH_MAX_CHARS_CEILING)),
            "marker missing: {:?}",
            &result.content[..result.content.len().min(200)]
        );
        // Same rule at the resolver level (cheap pin of the clamp semantics).
        assert_eq!(
            max_chars_from(&json!({"max_chars": 250_000u64})).unwrap(),
            WEB_FETCH_MAX_CHARS_CEILING
        );
        assert_eq!(max_chars_from(&json!({})).unwrap(), WEB_FETCH_DEFAULT_MAX_CHARS);
        assert_eq!(max_chars_from(&json!({"max_chars": 1u64})).unwrap(), 1);
        assert!(max_chars_from(&json!({"max_chars": "lots"})).is_err());
    }

    // ---------- leg 9 (non-vacuousness): the default cap is real ----------

    #[test]
    fn default_cap_is_20_000_chars_pinned_by_a_body_just_over() {
        // A cap-removal mutant returns all 20_050 chars and no marker; the
        // pinned default returns exactly 20_000 plus the marker.
        assert_eq!(WEB_FETCH_DEFAULT_MAX_CHARS, 20_000);
        let body = "z".repeat(WEB_FETCH_DEFAULT_MAX_CHARS + 50);
        let (url, stub) = serve(1, move |_req| StubResp::text(Some("text/plain"), &body));
        let result = fetch_via_dispatch(&format!("{url}/default"), None);
        stub.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(
            result.content.chars().filter(|c| *c == 'z').count(),
            20_000,
            "default cap must truncate (no cap → mutant survives)"
        );
        assert!(
            result.content.contains("[truncated at 20000 chars]"),
            "marker missing: {:?}",
            result.content
        );
    }

    // ---------- leg 6: non-http schemes refused, naming the scheme ----------

    #[test]
    fn non_http_schemes_are_refused_naming_the_scheme() {
        // No stub: the scheme gate must fire before any network I/O.
        let result = fetch_via_dispatch("file:///etc/passwd", None);
        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("file"), "{}", result.content);

        let result = fetch_via_dispatch("ftp://example.invalid/pub/readme.txt", None);
        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("ftp"), "{}", result.content);
    }

    #[test]
    fn missing_url_field_is_a_tool_error() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let result = dispatch(&ctx, "web_fetch", &json!({}));
        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("url"), "{}", result.content);
    }

    // ---------- leg 7: binary content refused, naming the type ----------

    #[test]
    fn binary_content_type_is_refused_naming_the_type() {
        let (url, stub) = serve(1, |_req| StubResp {
            status: 200,
            headers: vec![(
                "content-type".to_string(),
                "application/octet-stream".to_string(),
            )],
            body: vec![0x00, 0x01, 0x02, 0xff, 0xfe],
        });
        let result = fetch_via_dispatch(&format!("{url}/blob"), None);
        stub.join().unwrap();
        assert!(result.is_error, "{}", result.content);
        assert!(
            result.content.contains("application/octet-stream"),
            "{}",
            result.content
        );
    }

    // ---------- leg 8: dead port — fast tool error, no retry storm, no panic ----------

    #[test]
    fn dead_port_fails_fast_as_tool_error() {
        // T31's probe: a port verified to REFUSE connections at handoff, and
        // re-verified immediately before the connect (TOCTOU fence).
        let port = dead_port();
        assert_dead_port(port);
        let t0 = Instant::now();
        let result = fetch_via_dispatch(&format!("http://127.0.0.1:{port}/x"), None);
        let elapsed = t0.elapsed();
        assert!(result.is_error, "{}", result.content);
        assert!(
            result.content.contains("connection failed"),
            "error must name the class: {}",
            result.content
        );
        // One attempt, no retry loop (the code has none to storm with): a
        // refused loopback connect is immediate, so even the tool's own 10s
        // connect budget is a loose ceiling — and that is still well under
        // the 120s bash cap.
        assert!(
            elapsed < WEB_FETCH_CONNECT_TIMEOUT,
            "connection-refused leg took {elapsed:?}"
        );
    }
}
