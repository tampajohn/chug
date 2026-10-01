//! T180 — `web_search` (F12 phase 1): the provider seam + the zero-config
//! DuckDuckGo HTML provider.
//!
//! `web_fetch` (T37) fetches a KNOWN url — it cannot discover one, and the
//! loop's evaluation/doctrine work regularly needs "find the page" before
//! "fetch the page". `web_search` is the discovery half, built like
//! `web_fetch`: hard bounds everywhere, honesty legs instead of silent
//! empties, and the transport core shared with it
//! ([`crate::webfetch::fetch_raw`]).
//!
//! - **Provider seam**: [`SearchProvider`] is selected from the
//!   `CHUG_WEB_SEARCH_PROVIDER` env, read PER CALL (never latched — tests can
//!   vary it). Phase 1 ships one variant, the zero-config DuckDuckGo HTML
//!   provider; phase 2's keyed providers (Brave, Tavily) plug in here.
//! - **DuckDuckGo leg**: GET the HTML endpoint
//!   (`https://html.duckduckgo.com/html/?q=<url-encoded query>`; the base url
//!   is overridable via the test-only `CHUG_WEB_SEARCH_BASE_URL` env, which is
//!   deliberately NOT named in the model-facing description) and parse result
//!   blocks by CLASS TOKEN — attribute order and quoting may vary. A
//!   redirector href's `uddg` parameter is percent-decoded to the real target.
//! - **Honesty legs**: non-2xx → tool error with a preview (mirrors
//!   `web_fetch`); timeouts name their phase; and a page with ZERO parsed
//!   result blocks is never a silent empty list — it says either "0 results
//!   for <query>" (the page itself says no results) or names the byte count
//!   and the possible markup drift.

use anyhow::{anyhow, bail};
use serde_json::{json, Value};

use crate::tools::ToolResult;

/// Default `max_results` (spec req 1).
const WEB_SEARCH_DEFAULT_MAX_RESULTS: usize = 5;
/// Hard ceiling on `max_results` — a larger request CLAMPS, never errors
/// (spec req 1).
const WEB_SEARCH_MAX_RESULTS_CEILING: usize = 10;
/// Fixed output char cap (spec req 1: a `max_chars` param is NOT required; a
/// fixed cap with a truncation note is the accepted shape — webfetch's
/// default-cap semantics).
const WEB_SEARCH_OUTPUT_CHAR_CAP: usize = 20_000;
/// The provider-selection env (spec req 2), read per call.
const PROVIDER_ENV: &str = "CHUG_WEB_SEARCH_PROVIDER";
/// The valid provider values (phase 1: one zero-config provider).
const VALID_PROVIDERS: &[&str] = &["duckduckgo"];
/// The DuckDuckGo HTML endpoint base (spec req 3) — no API key.
const DUCKDUCKGO_BASE_URL: &str = "https://html.duckduckgo.com/html/";
/// The TEST-ONLY base-url seam (spec req 6): read per call so the end-to-end
/// leg runs against a local stub. Deliberately NOT named in the tool
/// description (the `CHUG_DELEGATE_BIN` precedent).
const BASE_URL_ENV: &str = "CHUG_WEB_SEARCH_BASE_URL";
/// The class tokens this parser matches on (attribute order and quoting may
/// vary — the token is what is stable; spec req 3).
const RESULT_TITLE_CLASS: &str = "result__a";
const RESULT_SNIPPET_CLASS: &str = "result__snippet";
/// The no-results marker class (spec req 4a).
const NO_RESULTS_CLASS: &str = "no-results";

/// The user-facing timeout phrase, built from the SHARED transport constants
/// (`webfetch`'s — one deadline pair for every chug network tool), so the
/// strings that promise "connect 10s, total 30s" cannot drift from the
/// deadlines actually enforced (the T42 discipline).
fn timeout_phrase() -> String {
    format!(
        "connect {}s, total {}s",
        crate::webfetch::WEB_FETCH_CONNECT_TIMEOUT.as_secs(),
        crate::webfetch::WEB_FETCH_TOTAL_TIMEOUT.as_secs()
    )
}

/// JSON schema for the `web_search` tool, registered alongside the builtins.
pub fn schema() -> Value {
    let description = format!(
        "Search the web and return numbered results, each with a title, url, and one-line snippet (tags stripped, whitespace collapsed). Provider: the DuckDuckGo HTML endpoint — zero-config, no API key; override with the {PROVIDER_ENV} env var (valid values: {} — unset/empty means the default). GET only; {}; at most max_results results (default {WEB_SEARCH_DEFAULT_MAX_RESULTS}, clamped to 1..={WEB_SEARCH_MAX_RESULTS_CEILING} — a larger request clamps, never errors). Output is char-capped at {WEB_SEARCH_OUTPUT_CHAR_CAP} with a truncation note. Unlike the filesystem tools this reaches OUTSIDE the cwd sandbox by design — it is network, not filesystem. Live HTML scraping can break or be rate-limited at any time; those surface as tool errors, never silent empty results. Non-2xx statuses and transport failures return as tool errors, never aborts; one attempt, no retry.",
        VALID_PROVIDERS.join(", "),
        timeout_phrase(),
    );
    json!({
        "name": "web_search",
        "description": description,
        "input_schema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "The search query (required, non-empty)"},
                "max_results": {"type": "integer", "description": format!("Maximum number of results (default {WEB_SEARCH_DEFAULT_MAX_RESULTS}; a request above the {WEB_SEARCH_MAX_RESULTS_CEILING} ceiling is clamped down, not rejected)")}
            },
            "required": ["query"]
        }
    })
}

/// Dispatch entry for the `web_search` arm in `tools::inner`.
pub fn web_search(input: &Value) -> anyhow::Result<ToolResult> {
    let query = query_from(input)?;
    let max_results = max_results_from(input)?;
    let provider = SearchProvider::from_env()?;
    let (page, body_truncated) = provider.search_page(&query)?;
    let hits = parse_results(&page, max_results);
    Ok(ToolResult {
        content: render(&query, &page, &hits, body_truncated),
        is_error: false,
        images: Vec::new(),
    })
}

/// The `query` field: required, a string, non-empty.
fn query_from(input: &Value) -> anyhow::Result<String> {
    let q = input
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing or non-string field: query (required, non-empty)"))?;
    if q.trim().is_empty() {
        bail!("query must be non-empty");
    }
    Ok(q.to_string())
}

/// Resolve `max_results`: default [`WEB_SEARCH_DEFAULT_MAX_RESULTS`] when
/// absent; 1..=[`WEB_SEARCH_MAX_RESULTS_CEILING`] when present — a larger
/// request is a clamp, not an error, but 0/negative/non-integer is a tool
/// error naming the bound (spec req 1 / the test contract).
fn max_results_from(input: &Value) -> anyhow::Result<usize> {
    const BOUND: &str = "max_results must be an integer in 1..=10 (default 5; a larger request is clamped, never rejected)";
    match input.get("max_results") {
        None | Some(Value::Null) => Ok(WEB_SEARCH_DEFAULT_MAX_RESULTS),
        Some(v) => match v.as_i64() {
            Some(n) if n >= 1 => Ok((n as usize).min(WEB_SEARCH_MAX_RESULTS_CEILING)),
            _ => bail!("{BOUND}"),
        },
    }
}

/// The provider seam (spec req 2). Phase 1 ships exactly one variant; phase
/// 2's keyed providers (Brave, Tavily) plug in here as new variants with
/// their own env names and fetch/parse legs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchProvider {
    DuckDuckGo,
}

impl SearchProvider {
    /// Read PER CALL (spec req 2: no process-global latch — tests vary the
    /// env without hazards beyond the process-global env lock itself).
    /// Unset/empty → the default; an unknown value is a tool error naming
    /// the received value AND the valid values.
    fn from_env() -> anyhow::Result<Self> {
        match std::env::var(PROVIDER_ENV) {
            Err(_) => Ok(Self::DuckDuckGo),
            Ok(v) if v.is_empty() || v == Self::DuckDuckGo.name() => Ok(Self::DuckDuckGo),
            Ok(v) => bail!(
                "unsupported {PROVIDER_ENV} value {v:?} — valid values: {} (unset/empty = default)",
                VALID_PROVIDERS.join(", ")
            ),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::DuckDuckGo => "duckduckgo",
        }
    }

    /// Fetch the provider's search page for `query`; returns the RAW html
    /// (the parser needs class attributes — stripping would destroy them)
    /// and whether the wire cap cut it. The base url comes from
    /// [`BASE_URL_ENV`] when set (test seam), else the provider default.
    fn search_page(&self, query: &str) -> anyhow::Result<(String, bool)> {
        match self {
            Self::DuckDuckGo => {
                let base = env_or_default(BASE_URL_ENV, DUCKDUCKGO_BASE_URL);
                // `query_pairs_mut` percent-encodes form-style (space → '+')
                // and replaces any query the base already carried — the base
                // is a URL prefix, not a full template.
                let mut url = reqwest::Url::parse(&base)
                    .map_err(|e| anyhow!("invalid {BASE_URL_ENV} value {base:?}: {e}"))?;
                url.query_pairs_mut().append_pair("q", query);
                crate::webfetch::fetch_raw(url.as_str(), "web_search")
            }
        }
    }
}

/// Env value or the default when unset/empty (same unset/empty semantics as
/// the provider env, spec reqs 2 and 6).
fn env_or_default(env: &str, default: &str) -> String {
    match std::env::var(env) {
        Ok(v) if !v.is_empty() => v,
        _ => default.to_string(),
    }
}

/// One parsed result block.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchHit {
    title: String,
    url: String,
    snippet: Option<String>,
}

/// One parsed `<a …>…</a>` anchor: byte spans and its `href`.
struct Anchor {
    /// Index of the opening `<`.
    tag_start: usize,
    /// Inner-text span: just past the opening `>` .. the matching `</a>`.
    inner: std::ops::Range<usize>,
    /// Index just past the closing `</a …>`.
    end: usize,
    /// The anchor's `href` attribute value, if present.
    href: Option<String>,
}

/// Parse at most `max_results` DuckDuckGo result blocks from the raw page.
/// The block shape: an `<a class="result__a" …>TITLE</a>` anchor plus the
/// sibling `<a class="result__snippet" …>SNIPPET</a>` anchor that follows it
/// before the NEXT title anchor (so a result without a snippet never
/// swallows the next block's — spec req 3 / the test contract).
fn parse_results(html: &str, max_results: usize) -> Vec<SearchHit> {
    let mut hits = Vec::new();
    let mut pos = 0usize;
    while hits.len() < max_results {
        let Some(title) = find_anchor_with_class(html, pos, RESULT_TITLE_CLASS) else {
            break;
        };
        let next_title = find_anchor_with_class(html, title.end, RESULT_TITLE_CLASS);
        let snippet = find_anchor_with_class(html, title.end, RESULT_SNIPPET_CLASS)
            .filter(|sn| next_title.as_ref().is_none_or(|nt| sn.tag_start < nt.tag_start))
            .map(|sn| visible_text(&html[sn.inner.clone()]));
        hits.push(SearchHit {
            title: visible_text(&html[title.inner.clone()]),
            url: resolve_url(title.href.as_deref().unwrap_or("")),
            snippet,
        });
        pos = title.end;
    }
    hits
}

/// Find the first `<a>` anchor at/after `from` whose class attribute carries
/// `token` as a whitespace-separated member. Matching is on the class TOKEN
/// (spec req 3): attribute ORDER (`class` first or `href` first) and quoting
/// vary, so no byte-exact prefix is assumed.
fn find_anchor_with_class(html: &str, from: usize, token: &str) -> Option<Anchor> {
    let (lt, gt) = find_tag_with_class(html, from, token, Some("a"))?;
    let b = html.as_bytes();
    let attrs = parse_attrs(b, lt + 2, gt);
    let href = attrs
        .iter()
        .find(|(name, _)| name == "href")
        .map(|(_, value)| value.clone());
    let close = find_close_anchor(b, gt);
    let end = close.map_or(b.len(), |c| crate::webfetch::skip_tag(b, c).unwrap_or(b.len()));
    Some(Anchor {
        tag_start: lt,
        inner: gt..close.unwrap_or(b.len()),
        end,
        href,
    })
}

/// Find the first OPEN tag at/after `from` whose class attribute carries
/// `token` as a whitespace-separated member; returns the `<` index and the
/// index just past the tag's `>`. `name` (when given) must also match the
/// tag name case-insensitively. A token occurrence in prose or inside
/// another attribute's value is refused, because the enclosing tag's PARSED
/// class attribute must actually carry the token.
fn find_tag_with_class(
    html: &str,
    from: usize,
    token: &str,
    name: Option<&str>,
) -> Option<(usize, usize)> {
    let b = html.as_bytes();
    let mut i = from;
    while let Some(occ) = find_sub_from(b, i, token.as_bytes()) {
        // `occ` is ABSOLUTE (find_sub_from returns `p + from`).
        // Walk back to the nearest '<' and accept only if it opens a tag
        // whose parsed class carries the token.
        if let Some(lt) = b[..occ].iter().rposition(|&c| c == b'<') {
            let name_ok = match name {
                None => true,
                Some(want) => tag_name_is(b, lt, want),
            };
            if name_ok
                && let Some(gt) = crate::webfetch::skip_tag(b, lt)
            {
                let has_token = parse_attrs(b, lt + 2, gt)
                    .iter()
                    .filter(|(attr, _)| attr == "class")
                    .any(|(_, value)| value.split_ascii_whitespace().any(|t| t == token));
                if has_token {
                    return Some((lt, gt));
                }
            }
        }
        i = occ + token.len();
    }
    None
}

/// True when `b[lt] == '<'` opens a tag named `name` (case-insensitive):
/// `<a …>`, `<a>`, `<A …>` — not `</a>` and not `<abbr …>`.
fn tag_name_is(b: &[u8], lt: usize, name: &str) -> bool {
    let mut j = lt + 1;
    for want in name.bytes() {
        match b.get(j) {
            Some(&c) if c.eq_ignore_ascii_case(&want) => j += 1,
            _ => return false,
        }
    }
    match b.get(j) {
        Some(&c) => c == b'>' || c == b'/' || c.is_ascii_whitespace(),
        None => false,
    }
}

/// Parse the attributes of an opening tag into lowercase-name/value pairs.
/// `name_end` is just past the tag name, `gt` just past the tag's `>` (both
/// byte indices into `b`). Values may be double- or single-quoted or bare;
/// valueless attributes map to an empty string. (Starting at `lt + 2` — one
/// char past `<` + one char of name — works for ANY one-char name and, for
/// longer names, yields a harmless leading pseudo-attribute followed by the
/// real ones, since a tag name can never contain `=`.)
fn parse_attrs(b: &[u8], name_end: usize, gt: usize) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let mut i = name_end;
    while i < gt {
        while i < gt && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        if i >= gt {
            break;
        }
        let name_start = i;
        while i < gt && !b[i].is_ascii_whitespace() && b[i] != b'=' && b[i] != b'>' {
            i += 1;
        }
        if i == name_start {
            // Nothing name-like at i: it is the tag's closing `>` right after
            // a completed attribute (the NORMAL shape — `href="…">`) or a
            // stray `=`. Skip one byte; the scans below can never advance on
            // these bytes, so this is the loop's only exit ramp.
            i += 1;
            continue;
        }
        let name = String::from_utf8_lossy(&b[name_start..i]).to_ascii_lowercase();
        while i < gt && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let value = if i < gt && b[i] == b'=' {
            i += 1;
            while i < gt && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < gt && (b[i] == b'"' || b[i] == b'\'') {
                let quote = b[i];
                i += 1;
                let value_start = i;
                while i < gt && b[i] != quote {
                    i += 1;
                }
                let value = String::from_utf8_lossy(&b[value_start..i]).into_owned();
                if i < gt {
                    i += 1; // past the closing quote
                }
                value
            } else {
                let value_start = i;
                while i < gt && !b[i].is_ascii_whitespace() && b[i] != b'>' {
                    i += 1;
                }
                String::from_utf8_lossy(&b[value_start..i]).into_owned()
            }
        } else {
            String::new()
        };
        attrs.push((name, value));
    }
    attrs
}

/// The `<` of the first `</a>`-ish closing tag at/after `from`
/// (case-insensitive on the name; `</a >` with a space also matches).
fn find_close_anchor(b: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 3 < b.len() {
        if b[i] == b'<'
            && b[i + 1] == b'/'
            && (b[i + 2] == b'a' || b[i + 2] == b'A')
            && (b[i + 3] == b'>' || b[i + 3].is_ascii_whitespace())
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Index of `needle` in `haystack[from..]`, if present (absolute index).
fn find_sub_from(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from > haystack.len() || needle.is_empty() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// Strip tags and collapse whitespace (spec req 3) — webfetch's
/// `html_to_text` is exactly that (block-tag boundaries, entity decode,
/// single spaces, trimmed), already factored `pub(crate)` for reuse.
fn visible_text(html: &str) -> String {
    crate::webfetch::html_to_text(html)
}

/// Decode the real target out of a result href (spec req 3): DuckDuckGo wraps
/// outbound links in a redirector — `…/l/?uddg=<percent-encoded real
/// url>&…` — and a `uddg` QUERY PARAMETER, when present, is percent-decoded
/// to the target; any other HREF is used verbatim.
fn resolve_url(href: &str) -> String {
    let b = href.as_bytes();
    let mut i = 0usize;
    while let Some(at) = find_sub_from(b, i, b"uddg=") {
        // `at` is ABSOLUTE (find_sub_from returns `p + from`).
        if at > 0 && (b[at - 1] == b'?' || b[at - 1] == b'&') {
            let rest = &href[at + 5..];
            let end = rest.find('&').unwrap_or(rest.len());
            return percent_decode(&rest[..end]);
        }
        i = at + 5;
    }
    href.to_string()
}

/// Percent-decode a URL component: `%XX` hex pairs; `+` stays `+` (this
/// decodes a URL embedded in a URL — a real `+` in the target arrives
/// `%2B`-encoded, and treating `+` as space would corrupt it).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok();
            if let Some(byte) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// True when the page itself says no results (spec req 4a): DuckDuckGo's
/// no-results markup carries the `no-results` class token; the phrase in the
/// stripped text is accepted too (an honest "no results" page whose markup
/// drifted is still a no-results page, not drift).
fn page_says_no_results(page: &str) -> bool {
    find_tag_with_class(page, 0, NO_RESULTS_CLASS, None).is_some()
        || visible_text(page).to_ascii_lowercase().contains("no results")
}

/// Render the tool's OK text. Zero parsed blocks is NEVER a silent empty
/// list (spec req 4 — the failure mode this leg exists to prevent): the page
/// saying no results renders "0 results for <query>"; otherwise the markup
/// is unrecognized and the output names the byte count and the possible
/// drift. Non-empty output: numbered results, each `title`, `url`, one-line
/// snippet, then the honesty notes, char-capped at
/// [`WEB_SEARCH_OUTPUT_CHAR_CAP`] with webfetch's truncation-marker shape.
fn render(query: &str, page: &str, hits: &[SearchHit], body_truncated: bool) -> String {
    if hits.is_empty() {
        if page_says_no_results(page) {
            return format!("0 results for {query}");
        }
        return format!(
            "search page returned {} bytes but no result blocks parsed (the DuckDuckGo HTML shape may have changed)",
            page.len()
        );
    }
    let mut out = String::new();
    for (i, hit) in hits.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!("{}. {}\n{}", i + 1, hit.title, hit.url));
        if let Some(snippet) = &hit.snippet {
            out.push('\n');
            out.push_str(snippet);
        }
    }
    if body_truncated {
        out.push_str(&format!(
            "\n[response body cut at the {}-byte read cap; results may be incomplete]",
            crate::webfetch::WEB_FETCH_BODY_BYTE_CAP
        ));
    }
    if out.chars().count() > WEB_SEARCH_OUTPUT_CHAR_CAP {
        let mut capped: String = out.chars().take(WEB_SEARCH_OUTPUT_CHAR_CAP).collect();
        capped.push_str(&format!("\n[truncated at {WEB_SEARCH_OUTPUT_CHAR_CAP} chars]"));
        out = capped;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp_http::tests::{accept_conn, read_request, write_response_close, Observed};
    use crate::tools::{dispatch, ToolCtx};
    use serde_json::json;
    use std::ffi::OsStr;
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    // ---------- fixture: a DuckDuckGo-shaped page covering every parser leg ----------

    /// Three hand-written blocks (uddg redirector; direct href with the
    /// attribute ORDER reversed, single quotes, uppercase tag; a result with
    /// no snippet) plus nine generated blocks — twelve total, so the
    /// max_results clamp has something to cut at 10.
    fn fixture_page() -> String {
        let mut page = String::from("<html><body>\n");
        page.push_str(
            r#"<div class="result"><h2 class="result__title"><a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fguide&amp;rut=ab12">Example <b>Guide</b> &amp; Notes</a></h2><a class="result__snippet" href="/x">First   result &amp; more <i>details</i></a></div>"#,
        );
        page.push('\n');
        page.push_str(
            r#"<A HREF='https://docs.example.com/api' CLASS='result__a'>Docs</A><a class="result__snippet">Second &lt;snippet&gt;</a>"#,
        );
        page.push('\n');
        page.push_str(r#"<a class="result__a" href="https://bare.example.com/">Bare Result</a>"#);
        page.push('\n');
        for n in 4..=12 {
            page.push_str(&format!(
                "<a class=\"result__a\" href=\"https://r{n}.example.com/\">Result {n}</a><a class=\"result__snippet\">snippet {n}</a>\n"
            ));
        }
        page.push_str("</body></html>");
        page
    }

    // ---------- parser legs ----------

    #[test]
    fn parser_handles_redirector_order_variance_entities_and_missing_snippet() {
        let hits = parse_results(&fixture_page(), 10);
        assert_eq!(hits.len(), 10, "twelve blocks on the page, capped at 10");
        assert_eq!(
            hits[0].url, "https://example.com/guide",
            "uddg redirector decoded (the &amp; separator must not leak into the value)"
        );
        assert_eq!(
            hits[0].title, "Example Guide & Notes",
            "tags stripped, entities decoded, whitespace collapsed"
        );
        assert_eq!(hits[0].snippet.as_deref(), Some("First result & more details"));
        assert_eq!(
            hits[1].url, "https://docs.example.com/api",
            "direct href with reversed attribute order / single quotes / uppercase tag"
        );
        assert_eq!(hits[1].title, "Docs");
        assert_eq!(hits[1].snippet.as_deref(), Some("Second <snippet>"));
        assert_eq!(hits[2].url, "https://bare.example.com/");
        assert_eq!(
            hits[2].snippet, None,
            "a result without a snippet must not swallow the next block's"
        );
        assert_eq!(hits[9].url, "https://r10.example.com/");
    }

    #[test]
    fn redirector_uddg_needs_a_param_boundary_and_other_hrefs_pass_verbatim() {
        assert_eq!(
            resolve_url("//duckduckgo.com/l/?uddg=https%3A%2F%2Fa.example%2Fb%3Fc&rut=9"),
            "https://a.example/b?c"
        );
        assert_eq!(
            resolve_url("https://plain.example.com/x?y=1"),
            "https://plain.example.com/x?y=1",
            "no redirector → verbatim"
        );
        assert_eq!(
            resolve_url("https://example.com/studdg=1"),
            "https://example.com/studdg=1",
            "`uddg=` without a ?/& boundary is prose, not a parameter"
        );
        assert_eq!(percent_decode("a%2Bb+c"), "a+b+c", "`+` stays `+`");
    }

    #[test]
    fn empty_page_legs_distinguish_no_results_from_unrecognized_markup() {
        // 4a: the page itself says no results.
        let no_results =
            r#"<html><body><div class="no-results">No results for odd query.</div></body></html>"#;
        assert!(page_says_no_results(no_results));
        let hits = parse_results(no_results, 5);
        assert_eq!(render("odd query", no_results, &hits, false), "0 results for odd query");
        // The phrase alone (markup drifted) still counts as no-results.
        assert!(page_says_no_results("<p>Sorry — no results today</p>"));

        // 4b: bytes arrived, markup unrecognized — the byte count is named.
        let drifted = "<html><body><p>totally different markup</p></body></html>";
        assert!(!page_says_no_results(drifted));
        let hits = parse_results(drifted, 5);
        let rendered = render("q", drifted, &hits, false);
        assert!(
            rendered.contains(&format!("search page returned {} bytes", drifted.len())),
            "{rendered}"
        );
        assert!(rendered.contains("the DuckDuckGo HTML shape may have changed"), "{rendered}");
    }

    // ---------- provider selection (per-call env read) ----------

    /// Set/unset one env var. Callers hold `DELEGATE_ENV_LOCK` (the crate's
    /// ONE process-global env lock, T129) and restore the saved value before
    /// returning; SAFETY inside: the mutation is serialized by that lock.
    fn set_env(var: &str, value: Option<&OsStr>) {
        unsafe {
            match value {
                Some(v) => std::env::set_var(var, v),
                None => std::env::remove_var(var),
            }
        }
    }

    #[test]
    fn provider_env_is_read_per_call_with_unset_empty_explicit_and_unknown_legs() {
        let _timing = crate::testsupport::timing_guard();
        let _guard = crate::delegate::tests::DELEGATE_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let saved = std::env::var_os(PROVIDER_ENV);

        set_env(PROVIDER_ENV, None);
        assert_eq!(
            SearchProvider::from_env().unwrap(),
            SearchProvider::DuckDuckGo,
            "unset → duckduckgo"
        );
        set_env(PROVIDER_ENV, Some(OsStr::new("")));
        assert_eq!(
            SearchProvider::from_env().unwrap(),
            SearchProvider::DuckDuckGo,
            "empty → duckduckgo"
        );
        set_env(PROVIDER_ENV, Some(OsStr::new("duckduckgo")));
        assert_eq!(SearchProvider::from_env().unwrap(), SearchProvider::DuckDuckGo);
        set_env(PROVIDER_ENV, Some(OsStr::new("brave")));
        let err = SearchProvider::from_env().unwrap_err().to_string();
        assert!(err.contains("brave"), "names the received value: {err}");
        assert!(err.contains("duckduckgo"), "names the valid values: {err}");
        assert!(err.contains(PROVIDER_ENV), "{err}");
        // Per-call read: the same process flips back — no latch.
        set_env(PROVIDER_ENV, Some(OsStr::new("duckduckgo")));
        assert_eq!(SearchProvider::from_env().unwrap(), SearchProvider::DuckDuckGo);

        set_env(PROVIDER_ENV, saved.as_deref());
    }

    // ---------- input rules ----------

    #[test]
    fn max_results_rules_default_clamp_and_error_naming_the_bound() {
        assert_eq!(max_results_from(&json!({})).unwrap(), 5, "absent → 5");
        assert_eq!(max_results_from(&json!({"max_results": 3u64})).unwrap(), 3);
        assert_eq!(
            max_results_from(&json!({"max_results": 99u64})).unwrap(),
            10,
            "a larger request clamps, never errors"
        );
        for bad in [json!(0), json!(-3), json!("five"), json!(true), json!(2.5)] {
            let err = max_results_from(&json!({"max_results": bad}))
                .unwrap_err()
                .to_string();
            assert!(err.contains('1'), "names the lower bound: {err}");
            assert!(err.contains("10"), "names the ceiling: {err}");
        }
    }

    #[test]
    fn query_is_required_and_non_empty_via_the_real_dispatcher() {
        let tmp = TempDir::new().unwrap();
        let ctx = ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: Duration::from_secs(crate::tools::BASH_TIMEOUT_SECS),
        };
        for input in [json!({}), json!({"query": ""}), json!({"query": "   "}), json!({"query": 3})] {
            let result = dispatch(&ctx, "web_search", &input);
            assert!(result.is_error, "{input}");
            assert!(result.content.contains("query"), "{input}: {}", result.content);
        }
    }

    // ---------- schema pins ----------

    #[test]
    fn schema_carries_the_honesty_tokens_and_hides_the_test_seam() {
        let s = schema();
        assert_eq!(s["name"], "web_search");
        assert_eq!(s["input_schema"]["required"], json!(["query"]), "query required");
        let d = s["description"].as_str().unwrap();
        for token in [
            "DuckDuckGo HTML endpoint",
            "zero-config, no API key",
            "CHUG_WEB_SEARCH_PROVIDER",
            "valid values: duckduckgo",
            "GET only",
            "connect 10s, total 30s",
            "clamped to 1..=10",
            "rate-limited",
            "network, not filesystem",
        ] {
            assert!(d.contains(token), "description missing {token:?}: {d}");
        }
        assert!(
            !d.contains(BASE_URL_ENV),
            "the test-only base-url seam must NOT be in the model-facing description: {d}"
        );
        assert!(s["input_schema"]["properties"]["query"].is_object());
        assert!(s["input_schema"]["properties"]["max_results"].is_object());
    }

    // ---------- render cap ----------

    #[test]
    fn output_is_char_capped_at_20000_with_a_marker() {
        let big = "x".repeat(30_000);
        let hits = vec![SearchHit {
            title: "t".into(),
            url: "https://a.example/".into(),
            snippet: Some(big),
        }];
        let out = render("q", "", &hits, false);
        // The cap counts TOTAL chars (title/url lines included), and the
        // truncation marker is appended after the cut — so the output is the
        // cap plus the marker and no more (a cap-removal mutant returns all
        // 30000 x's with no marker and fails both asserts).
        assert_eq!(
            out.chars().count(),
            WEB_SEARCH_OUTPUT_CHAR_CAP + "\n[truncated at 20000 chars]".chars().count()
        );
        assert!(
            out.contains(&format!("[truncated at {WEB_SEARCH_OUTPUT_CHAR_CAP} chars]")),
            "marker missing"
        );
    }

    // ---------- end-to-end stub legs (T6 harness, bounded accepts) ----------

    struct StubResp {
        status: u16,
        body: Vec<u8>,
    }

    impl StubResp {
        fn html(body: &str) -> Self {
            StubResp {
                status: 200,
                body: body.as_bytes().to_vec(),
            }
        }
    }

    /// Serve `n` request/response exchanges on 127.0.0.1 (the mcp_http T6
    /// harness: bounded accept + socket timeouts; one connection per
    /// request). Join the handle so a handler panic fails the test.
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
                write_response_close(
                    &mut stream,
                    resp.status,
                    &[("content-type", "text/html")],
                    &resp.body,
                );
            }
        });
        (url, handle)
    }

    fn target_of(req: &Observed) -> String {
        req.request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("/")
            .to_string()
    }

    fn search_via_dispatch(input: Value) -> crate::tools::ToolResult {
        let tmp = TempDir::new().unwrap();
        let ctx = ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: Duration::from_secs(crate::tools::BASH_TIMEOUT_SECS),
        };
        dispatch(&ctx, "web_search", &input)
    }

    #[test]
    fn stubbed_search_renders_numbered_decoded_results_and_clamps_to_ten() {
        let _timing = crate::testsupport::timing_guard();
        let _guard = crate::delegate::tests::DELEGATE_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let expected_ua = format!("chug/{}", crate::build_info::VERSION);
        let (base, stub) = serve(1, move |req| {
            assert_eq!(req.request_line.split(' ').next(), Some("GET"), "GET only");
            assert_eq!(req.header("user-agent"), Some(expected_ua.as_str()), "shared fixed UA");
            let target = target_of(req);
            assert!(target.starts_with("/html/?q="), "target: {target}");
            assert!(target.contains("rust+web+search"), "form-encoded query: {target}");
            StubResp::html(&fixture_page())
        });
        let saved = std::env::var_os(BASE_URL_ENV);
        set_env(BASE_URL_ENV, Some(OsStr::new(&format!("{base}/html/"))));
        // 99 requested → clamped to 10 (assert the clamp in the stubbed leg).
        let result = search_via_dispatch(json!({"query": "rust web search", "max_results": 99}));
        set_env(BASE_URL_ENV, saved.as_deref());
        stub.join().unwrap();

        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("1. Example Guide & Notes"), "{}", result.content);
        assert!(
            result.content.contains("https://example.com/guide"),
            "decoded uddg url: {}",
            result.content
        );
        assert!(result.content.contains("https://docs.example.com/api"), "{}", result.content);
        assert_eq!(
            result.content.lines().filter(|l| l.starts_with("10. ")).count(),
            1,
            "the clamp produced a 10th result: {}",
            result.content
        );
        assert!(
            !result.content.contains("\n11. "),
            "blocks past the clamp must not render: {}",
            result.content
        );
    }

    #[test]
    fn stub_non_2xx_is_a_tool_error_with_a_preview() {
        let _timing = crate::testsupport::timing_guard();
        let _guard = crate::delegate::tests::DELEGATE_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let (base, stub) = serve(1, |_| StubResp {
            status: 503,
            body: "rate limited — slow down".as_bytes().to_vec(),
        });
        let saved = std::env::var_os(BASE_URL_ENV);
        set_env(BASE_URL_ENV, Some(OsStr::new(&format!("{base}/html/"))));
        let result = search_via_dispatch(json!({"query": "q"}));
        set_env(BASE_URL_ENV, saved.as_deref());
        stub.join().unwrap();

        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("503"), "{}", result.content);
        assert!(result.content.contains("rate limited — slow down"), "{}", result.content);
    }

    // ---------- transport-constant pin (T42 discipline) ----------

    #[test]
    fn transport_constants_match_the_shared_webfetch_deadlines() {
        assert_eq!(timeout_phrase(), "connect 10s, total 30s");
        assert_eq!(
            crate::webfetch::WEB_FETCH_CONNECT_TIMEOUT,
            Duration::from_secs(10)
        );
        assert_eq!(
            crate::webfetch::WEB_FETCH_TOTAL_TIMEOUT,
            Duration::from_secs(30)
        );
    }
}
