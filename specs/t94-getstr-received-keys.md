# T94 — `get_str` error names the received keys (alias self-correction)

check: cargo test --bin chug received

## Repo context

Cycle-53 eval §2 I3: the t88 glm impl sent `old_string` (the
Anthropic-canonical alias) to `edit_file` THREE times, got
`missing or non-string field: old` each time, self-reported a tool BUG in
its own decision record, and routed around via bash heredoc — transcript
forensics proved a model fumble the error text could not correct. The T88
fix gave `decision_log` received-keys diagnostics; this row generalizes
the same shape to the ONE param-extraction helper every tool uses:
`src/tools.rs:217`

```rust
pub(crate) fn get_str<'a>(input: &'a Value, key: &str) -> anyhow::Result<&'a str> {
    input
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing or non-string field: {key}"))
}
```

Sibling extraction helpers in tools.rs (check for `get_u64`, `get_bool`,
`get_path`-adjacent extraction — `get_path` delegates to `get_str`) ride
the same change where they construct their own miss errors.

## Requirements

1. On a miss, when `input` is a JSON object, the error names the received
   keys in sorted order:
   `missing or non-string field: old (received keys: new_string, old_string, path)`.
   When `input` is NOT an object, name its JSON type instead (the T88
   req-3 shape): `missing or non-string field: old (received: array)`.
   An empty object → `(received keys: none)`.
2. **Success path byte-identical** — zero behavior change when the key is
   present and a string.
3. Key-listing is bounded (a malicious/accidental 500-key object must not
   balloon the error: cap at the first ~12 sorted keys + `, … (+N more)`).
4. Every tools.rs helper that constructs its own "missing or non-string"
   style error gets the same treatment; one shared formatting helper, not
   N copies (the T37/T70 one-module logic, in-module here).
5. README: NO change (error-text refinement below the docs' altitude).

## Tests (new tests carry the `received` stem)

- `edit_file`-shaped input with `old_string` instead of `old` → error
  names `old` as missing AND lists `new_string, old_string, path` —
  the exact t88 fumble, proving one-iteration self-correction is possible.
- Non-object input (array, string, null) → JSON-type naming leg.
- Empty object → `(received keys: none)`.
- >12 keys → capped list with the `+N more` suffix.
- Success path: every pre-existing tools.rs test green UNMODIFIED (the
  byte-identical requirement is enforced by the existing suite).
- RED proof: revert the helper change → the new tests fail.

## Acceptance

- `cargo test --bin chug received` green and covers every new leg (T96
  lesson: verify the run list).
- Full suite + clippy green; diff confined to `src/tools.rs`.
- Commit message records the RED proof + the t88-impl forensic citation
  (cycle-51 Outcomes "Forensic bonus").
