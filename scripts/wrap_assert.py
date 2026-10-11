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

Two further post-adoption fires extended the class into the count-ARG
shapes this module's T288 variant closes:

  d1791684611-3 (the cycle-508 wrap) — the remembered constant rode
  assert_count_exact's imported expected-count ARG (expected 6 vs probed
  8; 8 = 6 real entries + 2 legitimate pre-existing mentions), the
  remembered-ARG sub-shape.

  The cycle-512 wrap's 03:02:56Z leg — assert_count_exact('ring lines in
  the temp file') expected the ring's schema capacity 12 as a remembered
  ARG while the probe split the WHOLE temp file on `startswith('- d')`
  and read 17 (12 ring lines + 5 open-threads bullets). The expectation
  was right and the artifact correct; the probe's surface was wider than
  the section it named.

T289 closes the ring's duplication tooth-gap (the cycle-520 wrap's
rewrite): the corrupted 12-line decisions ring (8 unique decision ids,
the new batch's own head written twice into the tail) passed every
existing guard — the count gate reads 12 either way, the structural
self-check reads headers only, and the position-free carry check is
vacuous for a duplicated tail — so the ring surface learns
assert_ring_unique plus the position-aware assert_ring_splice.

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
    "ring_section_lines",
    "assert_ring_count",
    "assert_ring_unique",
    "assert_ring_splice",
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
    T287 made that blessing the doctrine's letter: the eval-delta splice
    gate and the ring discipline now pin the same prepend convention, and
    this position-free subsequence check is correct under it unchanged.
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


# RING_HEADER_PREFIX — the ring section's LINE-ANCHORED header anchor (T266).
# `^## decisions ring`, never a bare substring: the cycle-305 fire's splice
# anchored on `## open-threads` as a substring and matched INSIDE another
# field's prose cross-reference.
RING_HEADER_PREFIX = "## decisions ring"

# _ring_section_bounds(lines, who, where) — INTERNAL; e.g. _ring_section_bounds(lines, "assert_ring_count", "state.md")
def _ring_section_bounds(lines, who, where):
    """Locate the `## decisions` ring section; the T288 shapes' mechanics.

    The header anchor is LINE-ANCHORED (T266): a line STARTING WITH the
    ring's own heading text (RING_HEADER_PREFIX) — a bare substring that can
    match inside another field's prose is the banned anchor shape. The
    section runs from the line AFTER the header through the next line
    starting `## ` (exclusive) or EOF. Returns (header_index, lo, hi) —
    0-based bounds into *lines*, the body being lines[lo:hi] with BOTH
    boundary headers excluded. A missing header raises AssertionError: a
    genuinely-wrong input (the header is format-pinned), the anchor-miss
    guard firing correctly — not a wrong-expectation miscalibration.
    """
    header = None
    for i, line in enumerate(lines):
        if line.startswith(RING_HEADER_PREFIX):
            header = i
            break
    if header is None:
        raise AssertionError(
            f"{who}({where}): the ring header anchor {RING_HEADER_PREFIX!r} "
            f"matches NO line start in the probed artifact ({len(lines)} "
            f"line(s)) — a genuinely-wrong input, the line-anchored "
            f"anchor-miss guard firing (not a wrong-expectation "
            f"miscalibration)"
        )
    end = len(lines)
    for j in range(header + 1, len(lines)):
        if lines[j].startswith("## "):
            end = j
            break
    return header, header + 1, end


# _ring_capacity(header_line) — INTERNAL; e.g. _ring_capacity("## decisions ring (last 12, ...)")
def _ring_capacity(header_line):
    """Probe the ring's capacity from the header's own `last N` annotation.

    The probe-deriving primitive behind assert_ring_count: the EXPECTATION
    is read back from the live artifact's header at call time
    (`## decisions ring (last 12, ...)` probed as 12), never passed as a
    remembered constant. Scans every `last ` occurrence for the first one
    followed by digits (a prose `last ` without a number is skipped, not a
    false None). Returns None when the header carries no parseable `last N`
    — the caller's expected_capacity then fills in, and the failure message
    NAMES that fallback provenance.
    """
    marker = "last "
    idx = header_line.find(marker)
    while idx != -1:
        digits = ""
        for ch in header_line[idx + len(marker):]:
            if not ch.isdigit():
                break
            digits += ch
        if digits:
            return int(digits)
        idx = header_line.find(marker, idx + 1)
    return None


# _where(source) — INTERNAL; e.g. _where("state.md") -> "'state.md'"; _where(["a"]) -> "3 probed line(s)"
def _where(source):
    """A compact source label for failure messages (never a full-list repr)."""
    if isinstance(source, str) or hasattr(source, "__fspath__"):
        return repr(source)
    return f"{len(source)} probed line(s)"


# ring_section_lines(text_or_lines) — state-file TEXT (a str) or a line LIST; e.g. ring_section_lines(open(path).read())
def ring_section_lines(text_or_lines):
    """Extract the `## decisions` ring section's body lines (the T288 probe
    primitive).

    *text_or_lines* is EITHER the state-file TEXT (a str — split here,
    read_key_values' shape) OR an already-split line list (used directly,
    no re-split). Returns the lines BETWEEN the ring header and the next
    `## ` header (exclusive) or EOF — the section body only, both boundary
    lines excluded. THIS scoping is fire 4's fix: a `- d` count over the
    WHOLE file reads the open-threads bullets too (the cycle-512 wrap's
    03:02:56Z leg probed 17 = 12 ring lines + 5 `- d`-starting bullets and
    fired on structurally correct content); a count over THIS section
    cannot see them. A missing ring header raises (the anchor-miss guard).
    """
    if isinstance(text_or_lines, str):
        lines = text_or_lines.splitlines()
    else:
        lines = list(text_or_lines)
    _, lo, hi = _ring_section_bounds(lines, "ring_section_lines", _where(text_or_lines))
    return lines[lo:hi]


# assert_ring_count(path_or_lines, expected_capacity=12) — a PATH (str/os.PathLike) or a line LIST + the schema fallback; e.g. assert_ring_count("state.md")
def assert_ring_count(path_or_lines, expected_capacity=12):
    """The section-scoped, PROBE-DERIVED ring count (the T288 shape).

    Counts the `- d` lines WITHIN the `## decisions` ring section only
    (ring_section_lines' extraction — line-anchored on the ring's own
    header per T266, never a whole-file `startswith('- d')` split) and
    asserts the count equals the ring's capacity — the capacity PROBED from
    the section header's own `last N` annotation at call time. A remembered
    constant passed INTO the helper cannot set the expectation when the
    header probes one (fire 3, d1791684611-3: the remembered 6 rode the
    imported expected-count ARG and fired on a correct artifact — the probed
    capacity wins, so a stale ARG cannot mis-fire this gate).

    *path_or_lines* is EITHER a path (str/os.PathLike — opened and split
    here, assert_fields_non_empty's dispatch) OR an already-split line list
    (used directly, no re-read). *expected_capacity* is the FALLBACK for a
    header that probes no `last N` (provenance named in any failure) — it
    must be the schema's structural capacity from the format's own pinned
    text, never a count remembered from a previous artifact.

    The asserted invariant is the AT-CAPACITY ring (the wrap's post-write
    shape: prepend + rotate keeps the ring full — 11 or 13 ring lines is a
    REAL violation and fires). A legitimately under-filled YOUNG ring is a
    different call site: derive the expectation by probe (assert_count_exact
    with both sides probed, its 2-probed-counts contract).
    """
    if isinstance(path_or_lines, str) or hasattr(path_or_lines, "__fspath__"):
        with open(path_or_lines, "r", encoding="utf-8") as f:
            lines = f.read().splitlines()
    elif isinstance(path_or_lines, (list, tuple)):
        lines = list(path_or_lines)
    else:
        raise TypeError(
            f"assert_ring_count(path_or_lines, expected_capacity): "
            f"path_or_lines must be a PATH (a str or os.PathLike, opened and "
            f"split-read here) or an already-split LINE LIST (a list/tuple of "
            f"str, used directly, no re-read); probed source "
            f"{path_or_lines!r} is neither (type "
            f"{type(path_or_lines).__name__})"
        )
    header, lo, hi = _ring_section_bounds(
        lines, "assert_ring_count", _where(path_or_lines)
    )
    probed = _ring_capacity(lines[header])
    if probed is not None:
        capacity = probed
        provenance = (
            f"probed from the header's `last N` annotation "
            f"({lines[header]!r})"
        )
    else:
        capacity = expected_capacity
        provenance = (
            f"the expected_capacity fallback arg {expected_capacity!r} "
            f"(the header probed no `last N`) — the fallback must be the "
            f"schema's structural capacity, never a remembered count"
        )
    count = sum(1 for line in lines[lo:hi] if line.startswith("- d"))
    if count != capacity:
        raise AssertionError(
            f"assert_ring_count({_where(path_or_lines)}): probed ring-section "
            f"`- d` count {count} != capacity {capacity!r} ({provenance}) — "
            f"the count is scoped to the `## decisions` ring section (lines "
            f"{lo + 1}-{hi}), the expectation derived from the live artifact "
            f"at call time; a whole-file `- d` count mis-fires on the "
            f"open-threads bullets (fire 4, the cycle-512 wrap's 03:02:56Z "
            f"leg: expected 12 vs probed 17) and a remembered ARG cannot set "
            f"the expectation (fire 3, d1791684611-3)"
        )


# assert_ring_unique(path_or_lines) — a PATH (str/os.PathLike) or a line LIST; e.g. assert_ring_unique("state.md")
def assert_ring_unique(path_or_lines):
    """The ring's uniqueness contract (the T289 shape, the cycle-520 fire).

    Extracts the `## decisions` ring section (the T288/T266 LINE-ANCHORED
    extraction, never a bare substring) and asserts, within it: (a) no
    duplicated LINE byte-for-byte and (b) no duplicated decision id (the
    `- d<N>-<seq>:` prefix of each ring bullet). The cycle-520 wrap's
    rewrite landed a 12-line ring whose tail 4 lines are byte-duplicates
    of its head 4 (8 unique decision ids) and every existing guard passed
    it — assert_ring_count reads 12 either way, the structural self-check
    reads headers only, and the position-free carry check is VACUOUS for a
    duplicated tail. A duplicated carried line is corruption of the
    append-only history, never a rotation. The failure names EVERY
    duplicated id (or line repr) and BOTH offsets — ring-relative and
    file line numbers, probed from the artifact — in ONE AssertionError.
    Blank body lines are exempt from (a): they carry no decision content,
    a doubled blank is formatting noise, not a duplicated carried line.
    """
    if isinstance(path_or_lines, str) or hasattr(path_or_lines, "__fspath__"):
        with open(path_or_lines, "r", encoding="utf-8") as f:
            lines = f.read().splitlines()
    elif isinstance(path_or_lines, (list, tuple)):
        lines = list(path_or_lines)
    else:
        raise TypeError(
            f"assert_ring_unique(path_or_lines): path_or_lines must be a "
            f"PATH (a str or os.PathLike, opened and split-read here) or an "
            f"already-split LINE LIST (a list/tuple of str, used directly, "
            f"no re-read); probed source {path_or_lines!r} is neither "
            f"(type {type(path_or_lines).__name__})"
        )
    _, lo, hi = _ring_section_bounds(
        lines, "assert_ring_unique", _where(path_or_lines)
    )
    failures = []
    seen_lines = {}
    seen_ids = {}
    for i, line in enumerate(lines[lo:hi]):
        if line:
            if line in seen_lines:
                first = seen_lines[line]
                failures.append(
                    f"duplicated line {line!r} at ring lines {first + 1} and "
                    f"{i + 1} (file lines {lo + first + 1} and {lo + i + 1})"
                )
            else:
                seen_lines[line] = i
        if line.startswith("- d"):
            ring_id = line[2:].split(":", 1)[0].strip()
            if ring_id in seen_ids:
                first = seen_ids[ring_id]
                failures.append(
                    f"duplicated decision id {ring_id!r} at ring lines "
                    f"{first + 1} and {i + 1} (file lines "
                    f"{lo + first + 1} and {lo + i + 1})"
                )
            else:
                seen_ids[ring_id] = i
    if failures:
        raise AssertionError(
            f"assert_ring_unique({_where(path_or_lines)}): "
            f"{len(failures)} duplicated ring element(s) in the `## "
            f"decisions` ring section (lines {lo + 1}-{hi}) — " +
            "; ".join(failures) +
            " — a duplicated carried line is corruption of the append-only "
            "history, never a rotation (the cycle-520 wrap's rewrite: 12 "
            "ring lines / 8 unique decision ids, every count/structural "
            "guard passing it)"
        )


# assert_ring_splice(prev_ring_lines, new_ring_lines, prepended_count, capacity=12) — 2 line LISTS + a probed count
def assert_ring_splice(prev_ring_lines, new_ring_lines, prepended_count, capacity=12):
    """The position-aware ring-splice gate (the T289 shape).

    The composed ring must be PREPENDED + CARRIED structurally:
    new_ring_lines[prepended_count:] equals
    prev_ring_lines[0 : max(0, min(capacity - prepended_count,
    len(prev_ring_lines)))] BYTE-IDENTICAL and POSITION-ALIGNED —
    element-wise, never a subsequence (mirroring the eval-delta splice
    gate's shape: carried = the previous list's prefix below the new head;
    rotation drops from the previous ring's TAIL only). *prepended_count*
    is PROBED by the caller from the batch it composed (the T270/T273
    letter: probed actuals, never a remembered constant). The position-free
    assert_byte_carry STAYS for non-ring surfaces — its subsequence check
    is VACUOUS for a duplicated tail (the cycle-520 landing call verified
    the new batch's lines appear in the composed ring, which duplicates
    trivially satisfy), so the ring surface upgrades to this gate. The
    failure names the probed splice arithmetic (prepended, capacity, both
    lengths) or the first misaligned carried position with BOTH reprs.
    """
    if (
        not isinstance(prepended_count, int)
        or isinstance(prepended_count, bool)
        or prepended_count < 0
    ):
        raise TypeError(
            f"assert_ring_splice(prev_ring_lines, new_ring_lines, "
            f"prepended_count, capacity): prepended_count must be a "
            f"non-negative int (the count PROBED from the batch the caller "
            f"composed, never a remembered constant); probed "
            f"prepended_count {prepended_count!r} is "
            f"{type(prepended_count).__name__}"
        )
    keep = max(0, min(capacity - prepended_count, len(prev_ring_lines)))
    expected_carried = list(prev_ring_lines[:keep])
    actual_carried = list(new_ring_lines[prepended_count:])
    if len(actual_carried) != len(expected_carried):
        raise AssertionError(
            f"assert_ring_splice: the composed ring's length broke the "
            f"splice arithmetic — probed {len(new_ring_lines)} ring line(s) "
            f"= {prepended_count} prepended + {len(actual_carried)} "
            f"carried, expected {prepended_count} + {keep} (capacity "
            f"{capacity!r}, previous ring {len(prev_ring_lines)} line(s), "
            f"keep = max(0, min(capacity - prepended, len(prev)))) — a "
            f"dropped or duplicated carried line is corruption of the "
            f"append-only history, never a rotation"
        )
    for offset, (expected, actual) in enumerate(
        zip(expected_carried, actual_carried)
    ):
        if expected != actual:
            raise AssertionError(
                f"assert_ring_splice: the composed ring broke at carried "
                f"position {offset} (ring offset "
                f"{prepended_count + offset}) — probed expected "
                f"{expected!r}, probed actual {actual!r} — the carried "
                f"window must be the previous ring's prefix below the new "
                f"head, byte-identical and position-aligned; rotation drops "
                f"from the previous ring's TAIL only"
            )
