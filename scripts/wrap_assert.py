#!/usr/bin/env python3
"""wrap_assert.py — T273: the shared probed-assertion helper for wrap/fill
verification scripts.

META-META-SPEC.md's T270 clause (probe-then-assert-delta) bans hardcoded-
constant expectations in fill/wrap-phase verification-helper assertions —
the assertion must read the property back from the artifact (split-read
non-empty) or compute the delta from the actuals in the same probe
(count == pre + added). As text the clause worked as hygiene, but the
helpers it governs were ad-hoc bash/python heredocs re-authored at every
wrap, and the banned shape re-fired twice post-adoption:

  d1791616594-11 (the cycle-442 wrap) — the maiden state write's own
  helper re-authored `len(line) > len(key) + 2`, a fixed length threshold
  that fired on the legitimate 1-char `schema: 1` value across two
  full-script attempts while every other assertion verified the content
  correct.

  d1791618821-7 / d1791619025-10 (the cycle-446 wrap) — the ctx-edit
  helper hardcoded `blocks == 76`, a remembered constant against a
  live-grown transcript (78+ blocks); the guard fired pre-write on
  CORRECT content, the file never touched, corrected one leg later to
  probed counts.

This module is the teeth (the T273 doctrine pointer in META-META-SPEC.md):
ONE dependency-free (stdlib-only) helper the wrap/fill arcs IMPORT for the
recurring shapes instead of re-authoring assertions inline. Every function
raises AssertionError with a message naming the PROBED actuals — never a
bare constant mismatch — so a failed assertion reports what the artifact
actually held. The arguments ARE the probed actuals: passing a remembered
constant INTO a helper is the same banned shape as inlining it.

Coverage (run by the check line): python3 -m unittest scripts/test_wrap_assert.py -v
"""

__all__ = [
    "read_key_values",
    "assert_fields_non_empty",
    "assert_count_delta",
    "assert_byte_carry",
    "assert_substituted",
    "assert_count_exact",
    "assert_window_count",
    "assert_window_clean",
]


# read_key_values(text) — TEXT, not a path; e.g. read_key_values(open(path).read())
def read_key_values(text):
    """Split-read every `key: value` line into {key: value} (the T270 shape).

    The probe primitive the non-empty check is defined against: the value
    is what `line.split(':', 1)[1].strip()` yields. A bare `key:` line maps
    to '' (the empty-value shape the check fires on); the LAST occurrence
    of a duplicated key wins, matching a re-read of a healed artifact.
    """
    fields = {}
    for line in text.splitlines():
        if ":" not in line:
            continue
        key, _, value = line.partition(":")
        fields[key.strip()] = value.strip()
    return fields


# assert_fields_non_empty(source, keys) — a PATH (str/os.PathLike) or an already-parsed MAPPING; e.g. assert_fields_non_empty(path, ["purpose", "schema"])
def assert_fields_non_empty(source, keys):
    """Every named `key:` line in *source* has a non-empty split-read value.

    *source* is EITHER a path (str/os.PathLike — opened and split-read here)
    OR an already-parsed mapping (a read_key_values result, used directly
    with no re-read — the T277 shape, after the `expected str ... not dict`
    slip fired three times). The dispatch is structural (str/__fspath__ vs.
    keys()+__getitem__; the module stays import-free) — any other shape
    raises TypeError naming both accepted shapes.

    The T270 split-read shape: `line.split(':', 1)[1].strip() != ''` — the
    assertion reads the value back from the artifact, it does not measure
    the line's length (the d1791616594-11 banned shape fired on the
    legitimate 1-char `schema: 1` value). A MISSING key fails too — a
    missing field is not a non-empty field. ALL failing keys are reported
    in one AssertionError (each named, with its probed actual) so the wrap
    fixes every field in one pass.
    """
    if isinstance(source, str) or hasattr(source, "__fspath__"):
        with open(source, "r", encoding="utf-8") as f:
            fields = read_key_values(f.read())
    elif callable(getattr(source, "keys", None)) and hasattr(source, "__getitem__"):
        fields = source  # already parsed — used directly, no re-read
    else:
        raise TypeError(
            f"assert_fields_non_empty(source, keys): source must be a PATH (a "
            f"str or os.PathLike, opened and split-read here) or an already-"
            f"parsed MAPPING (a dict / collections.abc.Mapping, used directly, "
            f"no re-read); probed source {source!r} is neither "
            f"(type {type(source).__name__})"
        )
    failures = []
    for key in keys:
        if key not in fields:
            failures.append(
                f"missing key {key!r} (no {key}: line in the probed artifact)"
            )
        elif fields[key] == "":
            failures.append(f"empty key {key!r} (the split-read probed '')")
    if failures:
        raise AssertionError(
            f"assert_fields_non_empty({source!r}): {len(failures)}/{len(keys)} "
            f"named field(s) failed the split-read — " + "; ".join(failures)
        )


# assert_count_delta(before, after, added) — 3 probed values, no path; e.g. assert_count_delta(before=76, after=78, added=2)
def assert_count_delta(before, after, added):
    """`after == before + added`, computed from the probed actuals (T265).

    All three arguments are values the caller probed at assertion time —
    the T265 shell-arithmetic discipline's python form
    (`echo $((pre + added))`). The failure names all three probed actuals
    and the arithmetic, never a bare constant mismatch.
    """
    if after != before + added:
        raise AssertionError(
            f"assert_count_delta: probed actuals disagree — after={after!r}, "
            f"before={before!r}, added={added!r} ({after!r} != "
            f"{before!r} + {added!r}); the delta is computed from the probed "
            f"actuals in the same probe, never a remembered constant"
        )


# assert_byte_carry(prev_lines, new_lines) — 2 line LISTS, no path; e.g. assert_byte_carry(prev.splitlines(), new.splitlines())
def assert_byte_carry(prev_lines, new_lines):
    """The carried segment is byte-identical (the T262/T266 splice gate).

    When a ring (the decisions ring, the state file) is spliced, every
    carried line must survive BYTE-IDENTICAL and IN ORDER in the new
    lines — at any offset, since the new head prepends above the carry.
    Greedy leftmost matching (the standard subsequence check) is exact
    here: any failure means no alignment restores the carry, i.e. a carry
    line was mutated, reordered, or dropped. The failure names the first
    carry line that broke (its probed repr) and the candidate line probed
    at the offset where the match was expected.
    """
    carry_from = 0
    for i, expected in enumerate(prev_lines):
        pos = None
        for j in range(carry_from, len(new_lines)):
            if new_lines[j] == expected:
                pos = j
                break
        if pos is None:
            candidate = new_lines[carry_from] if carry_from < len(new_lines) else None
            raise AssertionError(
                f"assert_byte_carry: carried segment broke at carry line {i} "
                f"(probed {expected!r}) — no byte-identical match at or after "
                f"new-line offset {carry_from} (probed candidate there: "
                f"{candidate!r}); a carried line must survive byte-identical, "
                f"in order"
            )
        carry_from = pos + 1


# assert_substituted(path, needle, expected_present=True) — a PATH string, not text; e.g. assert_substituted(path, "WRAP_HASH")
def assert_substituted(path, needle, expected_present=True):
    """The fill substitution's probed verification (the T262 token check).

    Probes the artifact for *needle* and asserts presence/absence per
    *expected_present*. The failure names the probed occurrence count —
    the actual read back from the file — against the expectation.
    """
    with open(path, "r", encoding="utf-8") as f:
        occurrences = f.read().count(needle)
    if expected_present and occurrences == 0:
        raise AssertionError(
            f"assert_substituted({path!r}): probed needle {needle!r} expected "
            f"PRESENT but found 0 occurrences — the substitution did not land"
        )
    if not expected_present and occurrences != 0:
        raise AssertionError(
            f"assert_substituted({path!r}): probed needle {needle!r} expected "
            f"ABSENT but found {occurrences} occurrence(s) — the token survives"
        )


# assert_count_exact(actual, expected, label="count") — 2 probed counts, no path; e.g. assert_count_exact(live, probed, "blocks")
def assert_count_exact(actual, expected, label="count"):
    """The enumeration-count assertion, with the probed actual named (T272).

    *actual* is the count probed from the live artifact; *expected* is the
    count derived in the SAME probe (both sides `jq ... | wc -l`-measured).
    The failure names the probed actual against the expected — the
    d1791618821-7 banned shape was `blocks == 76` remembered against a
    live-grown transcript of 78+; re-probe the live count instead.
    """
    if actual != expected:
        raise AssertionError(
            f"assert_count_exact({label!r}): probed actual {actual!r} != "
            f"expected {expected!r} — re-probe the live count; a remembered "
            f"constant is the banned shape (T270/d1791618821-7)"
        )


# _resolve_window(lines, start, end, path, who) — INTERNAL; e.g. _resolve_window(lines, "pacing-streak:", "purpose:", path, "assert_window_count")
def _resolve_window(lines, start, end, path, who):
    """Locate the shared window; the T278 fill-phase shapes' mechanics.

    The window runs from the FIRST line containing the literal substring
    *start* (inclusive) through the first SUBSEQUENT line containing the
    literal substring *end* (exclusive), or to EOF when *end* is None.
    Returns (lo, hi, start_line, end_line) — the 0-based slice bounds into
    *lines* plus the 1-based anchor line numbers for reporting (end_line
    None at EOF). A missing anchor raises AssertionError naming it: a
    genuinely-wrong input (a typo'd anchor), the guard firing correctly —
    not a wrong-expectation miscalibration.
    """
    start_line = None
    for i, line in enumerate(lines):
        if start in line:
            start_line = i
            break
    if start_line is None:
        raise AssertionError(
            f"{who}({path!r}): the start anchor {start!r} matches NO line in "
            f"the probed artifact ({len(lines)} line(s)) — a genuinely-wrong "
            f"input, the anchor-miss guard firing (not a wrong-expectation "
            f"miscalibration)"
        )
    if end is None:
        return start_line, len(lines), start_line + 1, None
    end_line = None
    for j in range(start_line + 1, len(lines)):
        if end in lines[j]:
            end_line = j
            break
    if end_line is None:
        raise AssertionError(
            f"{who}({path!r}): the end anchor {end!r} matches NO line after "
            f"the start anchor {start!r} (start line {start_line + 1}, "
            f"{len(lines)} line(s) total) — a genuinely-wrong input, the "
            f"anchor-miss guard firing (not a wrong-expectation "
            f"miscalibration)"
        )
    return start_line, end_line, start_line + 1, end_line + 1


# _window_desc(start, start_line, end, end_line, lo, hi) — INTERNAL, all probed args; e.g. _window_desc("pacing-streak:", 9, "purpose:", 10, 8, 9)
def _window_desc(start, start_line, end, end_line, lo, hi):
    """The window's one-line span description for the failure messages."""
    if end_line is None:
        return (
            f"anchored on {start!r} (line {start_line}) through EOF "
            f"(lines {start_line}-EOF, {hi - lo} line(s))"
        )
    return (
        f"anchored on {start!r} (line {start_line}) through {end!r} "
        f"(exclusive, line {end_line}) (lines {start_line}-{end_line - 1}, "
        f"{hi - lo} line(s))"
    )


# assert_window_count(path, placeholder, start, expected, end=None) — a PATH string + a placeholder substring + 2 anchor substrings + a probed count; e.g. assert_window_count(path, "WRAP_HASH", "pacing-streak:", 1, end="purpose:")
def assert_window_count(path, placeholder, start, expected, end=None):
    """The fill substitution's count, WINDOW-SCOPED (the T278 shape).

    Reads the file at *path* and asserts *placeholder*'s occurrence count
    within the window == *expected*, where the window is the *start*
    anchor's line (inclusive) through the first subsequent *end*-anchor
    line (exclusive), or EOF when *end* is None.

    The trip-85 fill misfire (d1791646071-2): the hand-rolled heredoc
    asserted a FILE-WIDE count == 1 against the state file's TWO legitimate
    WRAP_HASH mentions — the pacing-line placeholder (the fill target) and
    the open-threads bullet's PROSE QUOTE of the token name — and fired
    twice pre-write on structurally correct content. Scoping the count to
    the fill target's window is the fix: prose that legitimately quotes
    the token name OUTSIDE the window never miscounts. Failures name the
    probed actual count, the expected, the placeholder, the anchor, and
    the window's line span. A window that matches NO line raises naming
    the missing anchor (the guard firing correctly).
    """
    with open(path, "r", encoding="utf-8") as f:
        lines = f.read().splitlines()
    lo, hi, start_line, end_line = _resolve_window(
        lines, start, end, path, "assert_window_count"
    )
    count = sum(line.count(placeholder) for line in lines[lo:hi])
    if count != expected:
        desc = _window_desc(start, start_line, end, end_line, lo, hi)
        raise AssertionError(
            f"assert_window_count({path!r}): probed placeholder {placeholder!r} "
            f"count {count} in the window != expected {expected!r} — window "
            f"{desc}; scope the count to the fill target's window — a "
            f"file-wide count misfires on prose that legitimately quotes "
            f"the token name (the trip-85 fill misfire, d1791646071-2)"
        )


# assert_window_clean(path, tokens, start, end=None) — a PATH string + a token LIST + an anchor substring; e.g. assert_window_clean(path, ["WRAP_HASH", "PROBE_RESULT"], "### Cycle 470", end="### ")
def assert_window_clean(path, tokens, start, end=None):
    """The post-fill bare-placeholder scan, WINDOW-SCOPED (the T278 shape).

    Same window mechanics as assert_window_count; asserts NONE of *tokens*
    appears within the window — the post-fill check that every placeholder
    was substituted. Tokens OUTSIDE the window (the carried entries'
    fill-instruction prose, the state's open-threads bullets) quote the
    names legitimately and are never scanned. The failure names EVERY
    found token with its window-relative line number in ONE AssertionError
    (the all-failures-in-one-error convention), so the wrap fixes every
    bare token in one pass.
    """
    with open(path, "r", encoding="utf-8") as f:
        lines = f.read().splitlines()
    lo, hi, start_line, end_line = _resolve_window(
        lines, start, end, path, "assert_window_clean"
    )
    findings = []
    for token in tokens:
        hits = [i for i in range(lo, hi) if token in lines[i]]
        if hits:
            label = "line" if len(hits) == 1 else "lines"
            window_lines = ", ".join(str(i - lo + 1) for i in hits)
            file_lines = ", ".join(str(i + 1) for i in hits)
            findings.append(
                f"{token!r} at window {label} {window_lines} "
                f"(file {label} {file_lines})"
            )
    if findings:
        desc = _window_desc(start, start_line, end, end_line, lo, hi)
        raise AssertionError(
            f"assert_window_clean({path!r}): {len(findings)} bare token(s) "
            f"still in the window ({desc}) — " + "; ".join(findings) +
            " — every placeholder must be substituted before the write"
        )
