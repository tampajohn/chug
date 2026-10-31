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

T295 adds validate_state — the ONE-call state-write gate facade (the
cycle-686 five-fire cluster's canonical fix): it owns the surface
conversions internally (read_key_values' MAPPING for the nine `state_field`
fields, the full LINES for count/unique with the capacity probed from the
ring header's own `last N`, ring_section_lines' extraction for the splice
and the corpus-tail identity), so a wrap's gate is `wa.validate_state(text)`
instead of ~7 hand-matched call-site surfaces re-derived per write (the
d1791921121-12 / d1791921226-14 pair passed RAW COMPOSED TEXT where the
helpers walk path-or-MAPPING / path-or-LINES → `OSError: File name too
long`, caught pre-write both times).

T306 adds the eval-commit probed-equality leg (the cycle-2035 filing
fire's write-time half): the 2031 wrap's state FULL recompose wrote
`eval-commit: 3930452b1f9e0e05a12e93d69a2d6ba95d7cbe47` — the 7-char
prefix TRUE, the suffix memory-composed (the TRUE full hash is
`39304522865f99ca95b6e8b8750e1714e627e7fb`) — and every existing guard
passed it, the field gated NON-EMPTY only; the falsification surfaced
one chain later at the next eval's boot delta read (fail-closed
FULL-READ, zero casualty). validate_state gains `expected_eval_commit`
— the T270/T273 probed-arg shape (`prev_ring_lines`, `corpus_tail_ids`
already ride): the CALL SITE probes `git rev-parse <commit>` and passes
the probed full hash in, the facade asserts the parsed field EQUALS it,
naming both values on mismatch; without the keyword the leg is inert.

T307 adds the state-drift first-token leg (the T304 arc's write-side
half, the cycle-2039 filing fire): the 2035 wrap's state FULL recompose
healed the eval-commit field (T306's leg rode green) but composed the
drift field with the healed event's NARRATIVE in the flag position —
`state-drift: FULL-READ REQUIRED (marker-unreachable) at the boot - …`
— and every existing guard passed it, the field gated NON-EMPTY only;
the 2039 boot's delta re-flagged FULL-READ REQUIRED
(state-drift-flagged) with the marker HEALED and reachable (fail-safe
by design, zero routing consequence; the structural hazard if unpinned:
every genuine drift event permanently latches the slim path off, each
wrap's narrative re-flagging the next boot). validate_state refuses a
drift field whose FIRST whitespace-token is not exactly `none` — an
INTRINSIC leg, no new keyword: the contract is the field's own (the
flag token first, the prose AFTER it — a healed/adjudicated drift event
writes `state-drift: none` with its narrative after the token, never
the narrative in the flag position), mirroring eval-delta.sh's
boot-time first-token compare write-side.

T309 adds the fill-datum leg (the corridor-figure falsification class's
4th-sighting guard, the cycle-2240 wrap): fill 8ea1079's committed FINAL
datum block read "72 iters / ~1,225.624s wall [2026-10-28T12:08:22.432Z ->
2026-10-28T12:28:48.056Z]" against the TRUE helper-generated record at the
stamp (73 iters / ~1,223.542s wall [-> 2026-10-28T12:28:45.974Z]) — the
figures composed from the PRIOR probe plus a projected delta, written into
the heredoc payload BEFORE the same leg's scripts/stream_datum.py run
printed the actuals (the compose-from-projection shape the T305 doctrine
bans; three fires in the last 28 cycles show the sentence alone does not
extinguish the habit). assert_fill_datum is the mechanical tooth: the fill
compose leg re-runs the helper at the fill stamp in the SAME pre-commit leg
and passes its stdout IN as the probed actual; the function asserts the
composed text carries that run's exact iters / wall-seconds / both stream
anchors — the compose-time catch over the same-wrap correction commit, the
fire zero-mutation by construction (both arguments are strs; nothing is
opened or written).

T310 adds the fill-enumeration leg (the T272 false-at-write class's newest
fire, the T309 datum leg's sibling surface): the cycle-2244 wrap fill
5b5e38f's committed FINAL datum block reads "ok:false 4 == 4 adjudicated"
at its named 2026-10-28T14:03:44Z stamp while the mechanical whole-final
enumeration of the stream finds FIVE `ok:false` legs at that stamp — the
fifth the 14:03:24.421Z assert_fill_datum AssertionError (the T309 guard's
first production fire, itself the designed pre-commit catch). The firing
sequence is the class's canonical shape: the compose leg's enumeration
probe ran BEFORE the datum-leg assert fired (4 legs at the probe), the
guard fire plus the recompose intervened, and the recomposed fill text
updated the datum figures (the T309 leg forced that) but NOT the
`ok:false` enumeration — a read taken earlier, not re-run before the write
(META-META-SPEC T272, the false-at-write shape). assert_fill_enumeration
is the mechanical tooth: the fill compose leg re-runs the enumeration at
the fill stamp in the SAME pre-commit leg (`jq -c 'select(.type==
"tool_result" and .ok==false)' .chug/events.jsonl | wc -l`, the count
captured into a shell variable per the T282 channel half — probed values
ride argv/env into the script, never literal-composed) and passes THAT
count in; the function asserts the composed text's FINAL-enumeration
rendering carries the probed count — the self-reference property (the
assert leg's own failure adds an `ok:false` leg) converging the same way
the datum leg's iteration count does: the recompose re-runs the probe and
asserts the NEW text carries the NEW count (5-after-fire vs 4-at-first-
probe on the 2244 stream).

T311 adds the on-disk state-schema leg (the 2515 false-GREEN class's
durable gate, the T309/T310 fill-assert family's third leg): the
cycle-2515 wrap's state recompose wrote `.chug/eval-state.md` in a
free-form prose format — ZERO of the nine pinned `state_field` fields
(`last-eval-commit:` substituted for `eval-commit:`) — every loopd delta
build since fails CLOSED (`FULL-READ REQUIRED: schema-mismatch`, the
enforcer working), yet the 2515 fill (a66f604) claimed "state write
FACADE-GREEN first attempt" while `validate_state` on the COMMITTED bytes
raises AssertionError at the FIELDS leg (RED-proven at the 597th eval) —
the claim composed without a real run over the on-disk bytes, and the
corridor had NO post-write mechanical check reading the on-disk state
through its actual consumer. assert_state_on_disk IS that run: it reads
the state file's BYTES from the probed path and runs the FULL
validate_state gate on the read text (expected_eval_commit forwarded
when supplied — probed via `git log -1 --format=%H -- EVALUATION.md`;
the splice/corpus-tail legs inert), wired into the fill compose leg's
pre-commit assert in the SAME leg as the commit.

Coverage (run by the check line): python3 -m unittest
scripts/test_wrap_assert.py -v — the pre-T295 legs plus the validate_state
facade legs, the T309 fill-datum legs, and the T310 fill-enumeration legs,
and the T311 on-disk state-schema legs.
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
    "validate_state",
    "assert_fill_datum",
    "assert_fill_enumeration",
    "assert_state_on_disk",
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

    T298 closes the zero-carry gap the cycle-1042 wrap's prepend-12/carry-0
    write slipped through: when the previous ring is NON-EMPTY and the
    composed ring is at capacity and the computed keep is 0, the
    carried-window assertion below would pass VACUOUSLY (0 expected
    carried, 0 actual — nothing to compare), so the shape is REFUSED here
    before it: T192's rule caps the rotation at capacity-1 so one prior
    line always survives verbatim, and a same-generation-only ring is
    corruption of the append-only history, never a rotation (the parity
    contract with eval-delta.sh's post-hoc `k == maxd && j == 0` leg —
    refused AT WRITE TIME, not one generation later). The remedy is the
    prepend-(capacity-1)/carry-1 splice: drop the batch's oldest new line
    and carry the previous ring's newest line as the tail. Below capacity
    (the under-capacity append path) and on an empty previous ring (the
    first-generation `m == 0` path) the current GREEN semantics hold.
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
    if prev_ring_lines and len(new_ring_lines) >= capacity and keep == 0:
        raise AssertionError(
            f"assert_ring_splice: the zero-carry full-ring write is "
            f"REFUSED — probed {prepended_count} prepended line(s) into a "
            f"capacity-{capacity} ring computes keep = max(0, "
            f"min(capacity - prepended, len(prev))) = 0, so the composed "
            f"{len(new_ring_lines)}-line ring carries ZERO of the previous "
            f"ring's {len(prev_ring_lines)} line(s) — T192: rotate at most "
            f"{capacity - 1} so one prior line always survives verbatim; "
            f"the remedy is to drop the batch's oldest new line and carry "
            f"the previous ring's newest line as the tail (prepend-"
            f"{capacity - 1}/carry-1) — a same-generation-only ring is "
            f"corruption of the append-only history (the cycle-1042 wrap's "
            f"prepend-12/carry-0 write passed GREEN through the vacuous "
            f"carried-window leg), never a rotation"
        )
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


# STATE_FIELD_KEYS — the nine `state_field` extractions the state parser's
# field check reads (scripts/eval-delta.sh: `schema` checked first, then the
# eight-key loop) and T266's structural self-check names ("all nine
# `state_field` extractions non-empty").
STATE_FIELD_KEYS = (
    "schema",
    "eval-commit",
    "eval-at",
    "state-drift",
    "fresh-input-last-eval",
    "cache-read-last-eval",
    "health",
    "open-threads",
    "pacing-streak",
)


# _ring_id(line) — INTERNAL; e.g. _ring_id("- d1791921121-12: the body") -> "d1791921121-12"
def _ring_id(line):
    """One ring line's decision id (the `- d<N>-<seq>:` prefix, T295)."""
    return line[2:].split(":", 1)[0].strip()


# _ring_ids(ring_lines) — INTERNAL; e.g. _ring_ids(ring_section_lines(text))
def _ring_ids(ring_lines):
    """A ring-body line list's decision ids, in order — the corpus-tail
    identity's probe primitive (T295). Only `- d` lines carry ids; blank
    body lines are skipped (the unique gate's own exemption)."""
    return [
        _ring_id(line) for line in ring_lines if line.startswith("- d")
    ]


# _probe_prepended(new_ring_lines, prev_ring_lines) — INTERNAL; 2 line LISTS
def _probe_prepended(new_ring_lines, prev_ring_lines):
    """Probe the batch's size from the two rings' OWN decision ids (T295).

    The prepend count assert_ring_splice demands is PROBED, never a
    remembered constant (the T270/T273 letter): the composed ring is
    batch-then-carried, so the batch is the LEADING RUN of ring lines whose
    decision ids are absent from the previous ring's ids — the scan stops at
    the first line whose id the previous ring already holds. A duplicated
    id (a batch line re-prepending an id the carry already holds) is
    corruption and stops the run early — the splice gate then fires on the
    misalignment with probed actuals, which is the gate working.
    """
    prev_ids = set(_ring_ids(prev_ring_lines))
    prepended = 0
    for line in new_ring_lines:
        if line.startswith("- d") and _ring_id(line) in prev_ids:
            break
        prepended += 1
    return prepended


# validate_state(text, prev_ring_lines=None, corpus_tail_ids=None, expected_eval_commit=None) — the composed state-file TEXT + 3 optional probed args
def validate_state(
    text, prev_ring_lines=None, corpus_tail_ids=None, expected_eval_commit=None
):
    """The ONE-call state-write gate (the T295 facade).

    A wrap's state-write gate composes ~7 helper calls and must match each
    helper's walk semantics BY HAND — assert_fields_non_empty takes
    path-or-MAPPING, assert_ring_count/assert_ring_unique take
    path-or-LINES, assert_ring_splice takes EXTRACTED ring lines, and the
    corpus-tail identity read goes through ring_section_lines. The
    cycle-686 wrap fired FIVE assertion slips at one write (d1791921121-12
    / d1791921226-14 the named pair: the RAW COMPOSED TEXT passed where the
    helper walks path-or-MAPPING / path-or-LINES → `OSError: File name too
    long`, caught pre-write) — one meta-root, call-site surfaces not swept
    against their consumers' walks. The facade is the canonical fix: ONE
    call owning the surface conversions internally, so no call site
    re-derives them.

    The gate, in order:
      1. FIELDS — read_key_values parses *text* and assert_fields_non_empty
         runs on the MAPPING (STATE_FIELD_KEYS, the nine `state_field`
         extractions the parser's field check reads), never on the raw text.
      2. STATE-DRIFT FIRST TOKEN — the parsed `state-drift` field's FIRST
         whitespace-token must be exactly `none` (an intrinsic leg, no new
         keyword: the contract is the field's own). eval-delta.sh's
         boot-time delta build reads the same first token and flags
         FULL-READ REQUIRED (state-drift-flagged) on anything else, so a
         healed/adjudicated drift event writes the token `none` with its
         narrative AFTER it — `none (the boot STATE-HIT: …)` — never the
         narrative in the flag position (the 2035 wrap's composition miss
         is the named counter-example: its narrative-first field re-flagged
         every later boot on the HEALED state, the permanent latch this
         leg closes). The mismatch names BOTH the received first token and
         the full field value (the T270 reporting requirement).
      3. EVAL-COMMIT PROBED EQUALITY (when *expected_eval_commit* is
         supplied) — the parsed `eval-commit` field must EQUAL the supplied
         probed full hash; the mismatch names BOTH the probed field value
         and the supplied probed actual (the T270 reporting requirement).
         The intended call shape: the wrap's state-write helper PROBES
         `git rev-parse <commit>` for the hash the state is being written
         against and passes the probed value in — never composing the hash
         from memory (the helper itself lives in the wrap corridor's shell,
         outside this repo's tracked code; the function contract is what
         lands here). When not supplied the leg is INERT — backwards-
         compatible with every existing call site (the field stays
         NON-EMPTY-checked only; eval-delta.sh's boot-time fail-closed
         marker check is the other half, unchanged).
      4. RING COUNT + UNIQUENESS — the full LINES feed assert_ring_count +
         assert_ring_unique, the capacity PROBED from the ring header's own
         `last N` (never a remembered 12 — the T288 letter).
      5. SPLICE (when *prev_ring_lines* is supplied) — the new ring is
         extracted via ring_section_lines and assert_ring_splice runs on
         the EXTRACTED lines: the prepended count PROBED from the two
         rings' own decision ids (_probe_prepended, never a remembered
         batch size), the capacity probed from the header (the schema's 12
         when the header carries no `last N`). Blank body lines are dropped
         from BOTH sides first — formatting noise, not carried content
         (the unique gate's own blank exemption) — so a section after the
         ring cannot false-fire the length arithmetic.
      6. CORPUS-TAIL IDENTITY (when *corpus_tail_ids* is supplied) — the
         extracted ring's decision ids must equal the supplied tail ids IN
         ORDER; the mismatch names the probed ring ids, the supplied tail
         ids, and the first divergence. The identity read goes through
         ring_section_lines — a bare whole-file `startswith` scan is the
         banned surface (the 105th fire matched five non-ring
         open-threads bullets ahead of the ring section).

    Failures propagate from the owning sub-helper (each naming its PROBED
    actuals — the module's invariant) or raise here (the identity leg); the
    facade adds no message of its own on their paths. The helpers' accepted
    shapes are NOT widened: raw text into assert_fields_non_empty /
    assert_ring_count still errors (the d1791921121-12 / d1791921226-14
    OSError shapes stay pinned) — the facade is the surface that accepts
    TEXT.

    *text* must be the composed state-file TEXT (a str) — a path or line
    list is the call-signature slip shape here (the conversions are the
    facade's job) and raises TypeError naming the accepted shape, as do
    non-list *prev_ring_lines* / *corpus_tail_ids* and non-str
    *expected_eval_commit* (the probed hash is `git rev-parse`'s output —
    a str; a non-str is the composing-from-memory slip shape the leg
    exists to close).
    """
    if not isinstance(text, str):
        probed = (
            f"{len(text)} probed line(s)"
            if isinstance(text, (list, tuple))
            else repr(text)
        )
        raise TypeError(
            f"validate_state(text, prev_ring_lines, corpus_tail_ids, "
            f"expected_eval_commit): text "
            f"must be the composed state-file TEXT (a str — the facade owns "
            f"the surface conversions internally: read_key_values' MAPPING "
            f"for the fields, the full LINES for count/unique, "
            f"ring_section_lines' extraction for the splice and the "
            f"corpus-tail identity); probed text {probed} (type "
            f"{type(text).__name__}) — passing a PATH or other non-str here "
            f"is the call-signature slip shape this facade closes "
            f"(d1791921121-12)"
        )
    if prev_ring_lines is not None and not isinstance(
        prev_ring_lines, (list, tuple)
    ):
        probed = (
            f"a {len(prev_ring_lines)}-char str"
            if isinstance(prev_ring_lines, str)
            else repr(prev_ring_lines)
        )
        raise TypeError(
            f"validate_state(text, prev_ring_lines, corpus_tail_ids, "
            f"expected_eval_commit): "
            f"prev_ring_lines must be the PREVIOUS ring's LINES (a "
            f"list/tuple of str — the previous state's ring_section_lines "
            f"extraction) or None; probed prev_ring_lines {probed} (type "
            f"{type(prev_ring_lines).__name__}) — a str here would walk as "
            f"CHARACTERS (the call-signature slip shape this facade closes)"
        )
    if corpus_tail_ids is not None and not isinstance(
        corpus_tail_ids, (list, tuple)
    ):
        probed = (
            f"a {len(corpus_tail_ids)}-char str"
            if isinstance(corpus_tail_ids, str)
            else repr(corpus_tail_ids)
        )
        raise TypeError(
            f"validate_state(text, prev_ring_lines, corpus_tail_ids, "
            f"expected_eval_commit): "
            f"corpus_tail_ids must be the corpus tail's decision ids IN "
            f"RING ORDER (a list/tuple of str, newest first — compared "
            f"element-wise against the ring section's probed ids) or None; "
            f"probed corpus_tail_ids {probed} (type "
            f"{type(corpus_tail_ids).__name__})"
        )
    if expected_eval_commit is not None and not isinstance(
        expected_eval_commit, str
    ):
        raise TypeError(
            f"validate_state(text, prev_ring_lines, corpus_tail_ids, "
            f"expected_eval_commit): expected_eval_commit must be the "
            f"PROBED full hash (a str — `git rev-parse <commit>`'s output, "
            f"probed by the wrap's state-write helper, never composed from "
            f"memory) or None; probed expected_eval_commit "
            f"{expected_eval_commit!r} (type "
            f"{type(expected_eval_commit).__name__}) — a non-str here is "
            f"the call-signature slip shape this facade closes"
        )
    # 1. FIELDS — the MAPPING surface (read_key_values parses here; the
    #    parsed mapping is reused by the eval-commit leg below).
    fields = read_key_values(text)
    assert_fields_non_empty(fields, STATE_FIELD_KEYS)
    # 2. STATE-DRIFT FIRST TOKEN — the intrinsic write-side half of the
    #    T304 first-token semantics: eval-delta.sh's boot-time delta build
    #    reads the field's FIRST whitespace-token and flags FULL-READ
    #    REQUIRED (state-drift-flagged) on anything but exactly `none`, so
    #    the flag position belongs to the token — a healed/adjudicated
    #    drift event is written `state-drift: none` with the narrative
    #    AFTER the token (`none (the boot STATE-HIT: …)`), never the
    #    narrative in the flag position (the 2035 wrap's composition miss:
    #    its narrative-first field re-flagged every later boot on the
    #    HEALED state — the permanent latch that would defeat T260's
    #    slim-path re-arm after the first drift).
    drift_value = fields.get("state-drift", "")
    drift_token = drift_value.split(None, 1)[0] if drift_value else ""
    if drift_token != "none":
        raise AssertionError(
            f"validate_state: the state-drift first-token leg broke — the "
            f"parsed `state-drift` field's first whitespace-token "
            f"{drift_token!r} != 'none' (the full field value "
            f"{drift_value!r}) — a healed/adjudicated drift event is "
            f"written `state-drift: none` with the narrative AFTER the "
            f"token (the T304 first-token semantics mirrored write-side: "
            f"eval-delta.sh's boot compare flags FULL-READ REQUIRED "
            f"(state-drift-flagged) on anything but the `none` token), "
            f"never the narrative in the flag position (the 2035 wrap's "
            f"composition miss wrote the healed event's narrative there "
            f"and every later boot re-flagged the HEALED state — the "
            f"permanent latch this leg exists to close)"
        )
    # 3. EVAL-COMMIT PROBED EQUALITY — the parsed field equals the probed
    #    hash the CALL SITE passed in (the call site probes `git rev-parse
    #    <commit>`; the facade only asserts the equality, naming BOTH
    #    values on mismatch — the T270 reporting requirement).
    if expected_eval_commit is not None:
        field_value = fields.get("eval-commit")
        if field_value != expected_eval_commit:
            raise AssertionError(
                f"validate_state: the eval-commit probed-equality leg broke "
                f"— the state's probed `eval-commit` field {field_value!r} "
                f"!= the supplied probed full hash "
                f"{expected_eval_commit!r} — the state-write call site must "
                f"probe `git rev-parse <commit>` and pass the probed value "
                f"in, never compose the hash from memory (the cycle-2035 "
                f"filing fire: the 2031 wrap's state FULL recompose wrote a "
                f"7-char-TRUE / suffix-memory-composed hash — `git "
                f"cat-file -t` on it rc-128 — latent across three cheap-exit "
                f"dispositions, caught by the boot delta's fail-closed "
                f"FULL-READ verdict; triage d1792968241-2)"
            )
    # 4. COUNT + UNIQUENESS — the LINES surface (each helper probes the
    #    capacity from the ring header's own `last N`).
    lines = text.splitlines()
    assert_ring_count(lines)
    assert_ring_unique(lines)
    # 5. SPLICE — the EXTRACTED-lines surface, blanks dropped both sides.
    if prev_ring_lines is not None:
        header, _, _ = _ring_section_bounds(
            lines, "validate_state", _where(lines)
        )
        probed_capacity = _ring_capacity(lines[header])
        capacity = probed_capacity if probed_capacity is not None else 12
        new_ring = [line for line in ring_section_lines(lines) if line != ""]
        prev_lines = [line for line in prev_ring_lines if line != ""]
        prepended = _probe_prepended(new_ring, prev_lines)
        assert_ring_splice(prev_lines, new_ring, prepended, capacity=capacity)
    # 6. CORPUS-TAIL IDENTITY — the extracted ring ids == the supplied tail.
    if corpus_tail_ids is not None:
        actual_ids = _ring_ids(ring_section_lines(lines))
        expected_ids = list(corpus_tail_ids)
        if actual_ids != expected_ids:
            divergence = None
            for k in range(min(len(actual_ids), len(expected_ids))):
                if actual_ids[k] != expected_ids[k]:
                    divergence = k
                    break
            if divergence is None:
                divergence = min(len(actual_ids), len(expected_ids))
            probed_at = (
                actual_ids[divergence] if divergence < len(actual_ids) else None
            )
            supplied_at = (
                expected_ids[divergence]
                if divergence < len(expected_ids)
                else None
            )
            raise AssertionError(
                f"validate_state: the corpus-tail identity broke — the ring "
                f"section's probed decision ids ({len(actual_ids)}) != the "
                f"supplied tail ids ({len(expected_ids)}), first divergence "
                f"at tail index {divergence} (probed {probed_at!r} vs "
                f"supplied {supplied_at!r}) — probed ring ids "
                f"{actual_ids!r}, supplied tail ids {expected_ids!r}; the "
                f"identity read goes through ring_section_lines (a bare "
                f"whole-file `startswith` scan is the banned surface — it "
                f"matched five non-ring open-threads bullets ahead of the "
                f"ring section, the 105th fire)"
            )


# _datum_figures(datum_output) — INTERNAL; e.g. _datum_figures(helper_stdout)
_FILL_DATUM_KEYS = ("iters", "first-anchor", "last-anchor", "wall-secs")


def _datum_figures(datum_output):
    """The four asserted figures read off the helper's `key: value` stdout.

    The read_key_values shape scoped to the datum keys (a `key: ` prefix
    match, so `ok-false-legs` never reads as `ok-false`). Fail-closed: a
    datum_output missing any figure — or carrying `none` for one (the
    helper's unparsable-anchor emission) — raises AssertionError naming the
    probed line(s); a datum without the figures cannot be asserted against,
    and a silent pass would wave a wrong-stream probe GREEN.
    """
    probed = {}
    for line in datum_output.splitlines():
        for key in _FILL_DATUM_KEYS:
            if line.startswith(key + ":"):
                probed[key] = line[len(key) + 1:].strip()
    problems = []
    for key in _FILL_DATUM_KEYS:
        value = probed.get(key)
        if value is None:
            problems.append(f"no `{key}:` line in the supplied datum_output")
        elif value == "" or value == "none":
            problems.append(f"the probed `{key}:` figure is {value!r}")
        elif key == "iters" and not value.isdigit():
            problems.append(
                f"the probed `iters:` figure {value!r} is not a digit count"
            )
    if problems:
        raise AssertionError(
            "assert_fill_datum: the supplied datum_output carries no "
            "assertable figure(s) — " + "; ".join(problems) + " — the "
            "helper output passed in must be a FRESH `python3 "
            "scripts/stream_datum.py <events> --at <fill-stamp>` stdout "
            "probed in the SAME pre-commit leg (iters / first-anchor / "
            "last-anchor / wall-secs all printed); a datum without the "
            "figures fails closed (never a silent pass)"
        )
    return probed


# _fill_datum_blocks(text) — INTERNAL; e.g. _fill_datum_blocks(composed_text)
_FILL_WS = " \t\r\n"
_FILL_TS_CHARS = "-T:.Z"


def _take_ts(text, p):
    """Consume the timestamp-shaped token at *p* ([0-9T:.Z-]+); (token, end)."""
    n = len(text)
    start = p
    while p < n and (text[p].isdigit() or text[p] in _FILL_TS_CHARS):
        p += 1
    return text[start:p], p


def _fill_datum_blocks(text):
    """The `N iters / ~W.WWWs wall [START -> END` blocks in *text*.

    Hand-scanned (the module stays import-free — no `re`): each literal
    `iters` marker reads as a datum block iff a digit count precedes it and
    the `/ ~WALL s wall [ ANCHOR -> ANCHOR` shape follows (whitespace
    tolerant, so a wrapped commit message still scans); any other
    occurrence is prose and skipped. Returns the (iters, wall, first, last)
    4-tuples in stream order.
    """
    blocks = []
    pos = 0
    n = len(text)
    while True:
        i = text.find("iters", pos)
        if i < 0:
            return blocks
        pos = i + len("iters")
        k = i
        while k > 0 and text[k - 1] in _FILL_WS:
            k -= 1
        d_end = k
        while k > 0 and text[k - 1].isdigit():
            k -= 1
        iters = text[k:d_end]
        if not iters:
            continue
        p = pos
        while p < n and text[p] in _FILL_WS:
            p += 1
        if p >= n or text[p] != "/":
            continue
        p += 1
        while p < n and text[p] in _FILL_WS:
            p += 1
        if p >= n or text[p] != "~":
            continue
        p += 1
        w_start = p
        while p < n and (text[p].isdigit() or text[p] in ",."):
            p += 1
        wall = text[w_start:p]
        if not wall or not any(c.isdigit() for c in wall):
            continue
        if p >= n or text[p] != "s":
            continue
        p += 1
        while p < n and text[p] in _FILL_WS:
            p += 1
        if not text.startswith("wall", p):
            continue
        p += 4
        while p < n and text[p] in _FILL_WS:
            p += 1
        if p >= n or text[p] != "[":
            continue
        first, p = _take_ts(text, p + 1)
        while p < n and text[p] in _FILL_WS:
            p += 1
        if not text.startswith("->", p):
            continue
        p += 2
        while p < n and text[p] in _FILL_WS:
            p += 1
        last, p = _take_ts(text, p)
        blocks.append((iters, wall, first, last))
        pos = p


# assert_fill_datum(text, datum_output) — 2 probed strings, no path; e.g. assert_fill_datum(fill_text, fresh_stream_datum_stdout)
def assert_fill_datum(text, datum_output):
    """The fill's FINAL datum figures equal a FRESH helper run's (T309).

    The corridor-figure falsification class's fourth sighting (the cycle-2240
    fill 8ea1079, corrected forward eae4d90): the FINAL datum block was
    composed from the PRIOR probe's figures plus a projected delta and
    written into the heredoc payload BEFORE the same leg's
    scripts/stream_datum.py run printed the actuals — the
    compose-from-projection shape the T305 doctrine bans.

    *datum_output* is the helper's OWN stdout (a str) from a FRESH
    `python3 scripts/stream_datum.py <events> --at <fill-stamp>` run at
    assertion time — passed IN as the probed actual (the T273 letter:
    arguments are the probed actuals; the function never re-derives the
    expectation from a remembered constant and never runs the helper
    itself). *text* is the composed fill/wrap TEXT (a str). Either non-str
    is the call-signature slip shape — TypeError naming the accepted shape.

    The asserted surface is EXACTLY the four figures the helper prints for
    the stream: the iterations count (`iters:`), the wall-seconds figure
    (`wall-secs:`), and BOTH stream anchors (`first-anchor:` /
    `last-anchor:`), each as the helper printed it. The block's other
    figures (ok:false / ctx-edit / trim / goal / bytes / mtime) and the
    display parenthetical derived FROM the wall figure (`PLAIN display 1223
    == 20*60+23`) are NOT asserted — asserting the derived parenthetical
    would double-count a figure the wall figure already pins.

    The block shape scanned in *text* is the corpus's FINAL-datum
    rendering, `N iters / ~W.WWWs wall [START -> END ...]`; the wall figure
    may carry the corpus's comma grouping (~1,223.542s) — normalized
    (commas stripped BOTH sides) before the compare — and the anchors
    compare exactly as printed. The assertion passes iff at least one block
    carries all four figures exactly (the fill carries ONE FINAL datum
    block; a text legitimately quoting an earlier corridor datum stays
    green because a FRESH run's stamp-figures cannot equal any earlier
    probe's block). Zero blocks is the named-missing fire — a datum-less
    fill is never a silent pass — and a datum_output missing any of the
    four figures fails closed naming the probed line. The fire is
    zero-mutation by construction: both arguments are strs, nothing is
    opened or written.
    """
    if not isinstance(text, str):
        raise TypeError(
            f"assert_fill_datum(text, datum_output): text must be the "
            f"composed fill/wrap TEXT (a str — the datum block the fill "
            f"commits is asserted in memory, pre-write); probed text "
            f"{text!r} (type {type(text).__name__})"
        )
    if not isinstance(datum_output, str):
        probed = (
            f"{len(datum_output)} probed line(s)"
            if isinstance(datum_output, (list, tuple))
            else repr(datum_output)
        )
        raise TypeError(
            f"assert_fill_datum(text, datum_output): datum_output must be "
            f"the helper's OWN stdout from a FRESH `python3 "
            f"scripts/stream_datum.py <events> --at <fill-stamp>` run (a "
            f"str — passed IN as the probed actual, never re-derived from a "
            f"remembered constant); probed datum_output {probed} (type "
            f"{type(datum_output).__name__})"
        )
    figures = _datum_figures(datum_output)
    blocks = _fill_datum_blocks(text)
    if not blocks:
        raise AssertionError(
            "assert_fill_datum: no FINAL datum block found in the composed "
            "fill text — zero `N iters / ~W.WWWs wall [START -> END ...]` "
            "blocks probed (the named-missing fire, never a silent pass); "
            "the fill must carry the datum block GENERATED by "
            "scripts/stream_datum.py at its named stamp (the T305 "
            "doctrine), its figures composed from the SAME pre-commit "
            "leg's fresh helper run — re-run the helper at the fill stamp "
            "and compose the block from its stdout"
        )
    reports = []
    for idx, (iters, wall, first, last) in enumerate(blocks, 1):
        divergent = []
        if iters != figures["iters"]:
            divergent.append(f"iters {iters!r} != probed {figures['iters']!r}")
        if wall.replace(",", "") != figures["wall-secs"].replace(",", ""):
            divergent.append(
                f"wall {wall!r} (comma-normalized {wall.replace(',', '')!r})"
                f" != probed {figures['wall-secs']!r}"
            )
        if first != figures["first-anchor"]:
            divergent.append(
                f"first-anchor {first!r} != probed {figures['first-anchor']!r}"
            )
        if last != figures["last-anchor"]:
            divergent.append(
                f"last-anchor {last!r} != probed {figures['last-anchor']!r}"
            )
        if not divergent:
            return
        reports.append(
            f"datum block {idx} of {len(blocks)} probed ({iters} iters / "
            f"~{wall}s wall [{first} -> {last}]): " + "; ".join(divergent)
        )
    raise AssertionError(
        f"assert_fill_datum: the fill text's datum block(s) diverge from "
        f"the helper's probed figures — " + " | ".join(reports) + " — the "
        "figures must be GENERATED by scripts/stream_datum.py at their "
        "named stamp and carried VERBATIM into the fill (the T305 "
        "doctrine; the cycle-2240 fill 8ea1079 composed 72/1,225.624 from "
        "the PRIOR probe plus a projected delta BEFORE the same leg's "
        "helper run printed the TRUE 73/1,223.542 — corrected forward "
        "eae4d90, the class's 4th sighting): re-run the helper at the fill "
        "stamp in the SAME pre-commit leg and compose the figures from its "
        "stdout, never from a remembered/projected figure"
    )


# _FILL_ENUM_PROBE — the mechanical enumeration command the CALL SITE runs
# (doctrine text, never executed here): the compose leg re-runs THIS at the
# fill stamp in the SAME pre-commit leg and passes the probed count in.
_FILL_ENUM_PROBE = (
    "jq -c 'select(.type==\"tool_result\" and .ok==false)' "
    ".chug/events.jsonl | wc -l"
)


# _enumeration_blocks(text) — INTERNAL; e.g. _enumeration_blocks(composed_text)
def _enumeration_blocks(text):
    """Every `ok:false K` enumeration figure in *text* — (head, tail) pairs.

    Hand-scanned (the module stays import-free — no `re`): each literal
    `ok:false` marker reads as an enumeration block iff whitespace then a
    digit count follows — the head figure K. The corpus's full
    FINAL-enumeration rendering carries ` == K'` after the head
    (`ok:false 4 == 4 adjudicated`); the tail figure is recognized and
    reported, the asserted figure is the head either way — the bare
    `ok:false K` head-figure (the establishment prose's shape,
    `ok:false 5, ctx 1, trim 0, goal 1, ...`) scans identically. Any
    other occurrence is prose and skipped: a non-digit follow
    (`ok:false count-0`), no following whitespace (`ok:false-legs`), the
    datum helper's hyphenated `ok-false:` keys. Whitespace-tolerant, so a
    wrapped commit message still scans. Returns (head, tail_or_None)
    string tuples in stream order.
    """
    blocks = []
    marker = "ok:false"
    pos = 0
    n = len(text)
    while True:
        i = text.find(marker, pos)
        if i < 0:
            return blocks
        pos = i + len(marker)
        p = pos
        if p >= n or text[p] not in _FILL_WS:
            continue
        while p < n and text[p] in _FILL_WS:
            p += 1
        start = p
        while p < n and text[p].isdigit():
            p += 1
        head = text[start:p]
        if not head:
            continue
        q = p
        while q < n and text[q] in _FILL_WS:
            q += 1
        tail = None
        if text.startswith("==", q):
            q += 2
            while q < n and text[q] in _FILL_WS:
                q += 1
            t_start = q
            while q < n and text[q].isdigit():
                q += 1
            if q > t_start:
                tail = text[t_start:q]
        blocks.append((head, tail))


# _fire_named_missing(probed_stamp) — INTERNAL; e.g. _fire_named_missing(fill_stamp)
def _fire_named_missing(probed_stamp):
    """The zero-block fire: a datum-less fill is never a silent pass (T310).

    Split from assert_fill_enumeration's body so the RED-provability
    mutant (the fire silent-passed) is a ONE-LINE swap on a copy — the
    guard `_fire_named_missing(probed_stamp)` -> `return` — rather than a
    multi-line raise excision that cannot compile half-removed.
    """
    raise AssertionError(
        f"assert_fill_enumeration: no `ok:false K` enumeration figure "
        f"found in the composed fill text — zero enumeration blocks "
        f"probed (the named-missing fire, never a silent pass); the "
        f"fill must carry the FINAL-enumeration rendering "
        f"(`ok:false K == K adjudicated`) at its named stamp "
        f"({probed_stamp!r}), its figure composed from the SAME "
        f"pre-commit leg's mechanical enumeration — re-run "
        f"`{_FILL_ENUM_PROBE}` at the fill stamp and compose the "
        f"figure from the probed count"
    )


# assert_fill_enumeration(text, probed_count, probed_stamp) — the composed TEXT + 2 probed actuals; e.g. assert_fill_enumeration(fill_text, probed_ok_false_count, fill_stamp)
def assert_fill_enumeration(text, probed_count, probed_stamp):
    """The fill's FINAL enumeration figure equals a FRESH mechanical count.

    The T272 false-at-write class's newest fire (the cycle-2244 wrap fill
    5b5e38f, corrected forward per T70 at the cycle-2248 eval's
    establishment): the committed FINAL datum block reads "ok:false 4 == 4
    adjudicated" at its named 2026-10-28T14:03:44Z stamp while the
    mechanical whole-final enumeration finds FIVE `ok:false` legs at that
    stamp — the fifth the 14:03:24.421Z assert_fill_datum AssertionError
    (the T309 guard's first production fire, itself the designed
    pre-commit catch). The compose leg's enumeration probe ran BEFORE the
    datum-leg assert fired (4 legs at the probe), the guard fire plus the
    recompose intervened, and the recomposed fill updated the datum
    figures but NOT the enumeration — a read taken earlier, not re-run
    before the write (META-META-SPEC T272, the false-at-write shape).

    *probed_count* is the `ok:false`-leg count the CALL SITE probed from
    the LIVE events stream at assertion time — the FRESH
    `{_FILL_ENUM_PROBE}` reading at the fill stamp, captured into a shell
    variable per the T282 channel half — passed IN as the probed actual
    (the T273 letter: arguments are the probed actuals; the function
    never re-derives the expectation nor runs the enumeration itself).
    *probed_stamp* is the stamp the enumeration was probed at (the fill
    stamp) — named in every fire so the recompose knows where to
    re-probe. *text* is the composed fill/wrap TEXT (a str). Either
    non-str is the call-signature slip shape — TypeError naming the
    accepted shape (the pre-split line list is the text slip shape).

    The asserted surface is the corpus's FINAL-enumeration rendering,
    `ok:false K == K` (and the bare `ok:false K` head-figure): the
    assertion passes iff at least ONE probed block carries the probed
    count exactly (any-block semantics, the datum leg's rule — a text
    legitimately quoting an earlier stream's figure stays green because
    the FINAL datum's own figure rides beside it). Zero blocks is the
    named-missing fire — a datum-less fill is never a silent pass — and a
    degenerate *probed_count* (non-int, negative) fails closed naming the
    probed value. The fire is zero-mutation by construction: all
    arguments are values, nothing is opened or written.
    """
    if not isinstance(text, str):
        probed = (
            f"{len(text)} probed line(s)"
            if isinstance(text, (list, tuple))
            else repr(text)
        )
        raise TypeError(
            f"assert_fill_enumeration(text, probed_count, probed_stamp): "
            f"text must be the composed fill/wrap TEXT (a str — the "
            f"enumeration figure the fill commits is asserted in memory, "
            f"pre-write); probed text {probed} (type "
            f"{type(text).__name__}) — the pre-split line list is the "
            f"call-signature slip shape"
        )
    if not isinstance(probed_stamp, str):
        probed = (
            f"{len(probed_stamp)} probed line(s)"
            if isinstance(probed_stamp, (list, tuple))
            else repr(probed_stamp)
        )
        raise TypeError(
            f"assert_fill_enumeration(text, probed_count, probed_stamp): "
            f"probed_stamp must be the stamp the mechanical enumeration "
            f"was probed at (a str — the fill stamp, named in every fire "
            f"so the recompose knows where to re-probe); probed "
            f"probed_stamp {probed} (type {type(probed_stamp).__name__})"
        )
    if (
        not isinstance(probed_count, int)
        or isinstance(probed_count, bool)
        or probed_count < 0
    ):
        raise AssertionError(
            f"assert_fill_enumeration: the probed count fails closed — "
            f"probed probed_count {probed_count!r} (type "
            f"{type(probed_count).__name__}) is not a non-negative int — "
            f"the count must be the LIVE `{_FILL_ENUM_PROBE}` reading at "
            f"the fill stamp probed in the SAME pre-commit leg (the T282 "
            f"channel half: probed values ride argv/env into the script, "
            f"never literal-composed); a degenerate probed count can "
            f"never wave the leg GREEN"
        )
    enum_blocks = _enumeration_blocks(text)
    if not enum_blocks:
        _fire_named_missing(probed_stamp)
    reports = []
    for idx, (head, tail) in enumerate(enum_blocks, 1):
        if int(head) == probed_count:
            return
        shape = (
            f"the full FINAL-enumeration rendering (== {tail})"
            if tail is not None
            else "the bare head-figure"
        )
        reports.append(
            f"enumeration block {idx} of {len(enum_blocks)} probed "
            f"({shape}: ok:false {head}): block figure {head!r} != probed "
            f"{probed_count}"
        )
    raise AssertionError(
        f"assert_fill_enumeration: the fill text's enumeration figure(s) "
        f"diverge from the mechanical enumeration probed at the fill "
        f"stamp {probed_stamp!r} — " + " | ".join(reports) + " — the "
        "figure must carry the SAME pre-commit leg's probed count, never "
        "a read taken earlier (the T272 false-at-write shape: the "
        "recompose-after-guard-fire updated the datum figures and never "
        "re-ran the enumeration — the cycle-2244 fill 5b5e38f wrote "
        "\"ok:false 4 == 4\" at its 14:03:44Z stamp against the mechanical "
        "5, the fifth leg the T309 datum guard's own first fire): re-run "
        f"`{_FILL_ENUM_PROBE}` at the fill stamp in the SAME pre-commit "
        "leg and compose the figure from the probed count"
    )


# _STATE_ON_DISK_PROBE — the mechanical eval-commit probe the CALL SITE runs
# (doctrine text, never executed here): the fill compose leg probes the
# commit the state was written against with THIS and passes the probed full
# hash in — never composed from memory (the T306 discipline, mirrored here).
_STATE_ON_DISK_PROBE = "git log -1 --format=%H -- EVALUATION.md"


# assert_state_on_disk(path, expected_eval_commit=None) — a PATH (str) + the optional probed hash; e.g. assert_state_on_disk(".chug/eval-state.md", probed_full_hash)
def assert_state_on_disk(path, expected_eval_commit=None):
    """The ON-DISK state-schema leg (T311 — the 2515 false-GREEN class's
    durable gate, the T309/T310 fill-assert family's third leg).

    The filing fire (the cycle-2515 wrap): the wrap's state recompose wrote
    `.chug/eval-state.md` in a free-form prose format — ZERO of the nine
    pinned `state_field` fields (`last-eval-commit:` substituted for
    `eval-commit:`, no `schema:`, no `eval-at:`, no `state-drift:`, no
    telemetry fields, no `health:`, no `open-threads:`, no
    `pacing-streak:`) and none of the doctrine's pinned sections — and
    every loopd delta build since fails CLOSED (`FULL-READ REQUIRED:
    schema-mismatch`, the enforcer working). But the 2515 fill (a66f604)
    claimed "state write FACADE-GREEN first attempt" while
    `validate_state` on the COMMITTED bytes raises AssertionError at the
    FIELDS leg (RED-proven at the 597th eval) — the facade CANNOT have
    greened the committed text; the claim was composed without a real run
    over the on-disk bytes (or over a draft a later leg overwrote). The
    corridor had NO post-write mechanical check that reads the on-disk
    state through its actual consumer, so a non-conformant write AND a
    false GREEN claim both passed silently.

    This leg IS that run, wired into the fill compose leg's pre-commit
    assert (the SAME leg as the commit, per the T309/T310 pattern — the
    compose-time catch independent of the writer's claim): a GREEN
    state-write claim is never composed without the mechanical run over
    the on-disk file.

    *path* is the state file's PATH (a str — the BYTES are read HERE, from
    disk, and the FULL validate_state gate runs on the read text); any
    other shape is the call-signature slip shape and raises TypeError
    naming the accepted shape. *expected_eval_commit*, when supplied, is
    forwarded to the facade's probed-equality leg (T306) — PROBED by the
    caller via `git log -1 --format=%H -- EVALUATION.md`, never composed
    from memory. The splice/corpus-tail legs stay INERT — the on-disk
    state stands alone (no previous-ring context is composed here; the
    ring's count/uniqueness legs still gate). Failures propagate from the
    facade (each naming its PROBED actuals — the module's invariant); a
    missing/unreadable file raises AssertionError naming the probed path
    and the error (never a bare traceback) — a missing state can never
    wave the fill's state-write claim GREEN. The fire is zero-mutation by
    construction: the file is only ever READ.
    """
    if not isinstance(path, str):
        probed = (
            f"{len(path)} probed line(s)"
            if isinstance(path, (list, tuple))
            else repr(path)
        )
        raise TypeError(
            f"assert_state_on_disk(path, expected_eval_commit): path must "
            f"be the ON-DISK state file's PATH (a str — the BYTES are read "
            f"HERE, from disk, and the FULL validate_state gate runs on the "
            f"read text, so the caller passes the probed path — "
            f"`{_STATE_ON_DISK_PROBE}` supplies the optional hash — never "
            f"the text itself, the facade's own surface); probed path "
            f"{probed} (type {type(path).__name__})"
        )
    try:
        with open(path, "rb") as f:
            raw = f.read()
        text = raw.decode("utf-8")
    except (OSError, UnicodeDecodeError) as err:
        raise AssertionError(
            f"assert_state_on_disk({path!r}): the on-disk state file is "
            f"unreadable — {type(err).__name__}: {err} — the leg reads the "
            f"committed bytes through the state's actual consumer "
            f"(validate_state), so a missing/unreadable state can never "
            f"wave the fill's state-write claim GREEN (the 2515 shape: the "
            f"claim composed without a real run over the on-disk bytes)"
        ) from err
    validate_state(text, expected_eval_commit=expected_eval_commit)
