# T53 — loopd single-driver check: replace dead pgrep with a ps-based check

One concern: `loopd.sh`'s single-driver guard uses `pgrep -f`, which on the
production host persistently cannot see the launchd-spawned loopd tree —
the guard has never matched an in-tree driver and fails OPEN (duplicate
drivers possible).

## Repo context

- `loopd.sh:98` (inside the `run` mode's `while [ ! -f "$STOP" ]` body,
  immediately after the T50 re-exec block and before `cargo build`):
  ```sh
  # Single-driver: never overlap another LOOP-SPEC run (e.g. a manual one).
  if pgrep -f "chug run --spec LOOP-SPEC.md" > /dev/null 2>&1; then
    echo "$(ts) another LOOP-SPEC driver active; skipping" >> "$LOG"
    sleep 120
    continue
  fi
  ```
- **Evidence (cycle-24 eval, reproduced live):** the orchestrator ran as
  pid 37073 (`./target/debug/chug run --spec LOOP-SPEC.md`, child of loopd
  pid 90114, parented to launchd). Across 9/9 invocations over ~10 minutes:
  `pgrep -f "chug run --spec LOOP-SPEC.md"`, `pgrep -f "chug run"`,
  `pgrep -l chug`, and even `pgrep -P 90114` (pattern-free parent match)
  ALL missed pid 37073; `pgrep -f .`'s full list omitted BOTH 37073 and
  90114 while `ps -ax` listed them every time (argv intact). The blindness
  is process-persistent for the launchd tree, not pattern-specific — a
  `nohup sleep 45` from the same shell matched `pgrep -f "sleep 45"`
  instantly. pgrep's visible-set also flapped (1298→1295→1293) with
  boot-time daemons dropping in and out. `.chug/loopd/loopd.log` shows the
  skip line fired exactly once in supervisor history (00:44:00Z, ~20s after
  a supervisor start — an out-of-tree driver).
- `tests/loopd_reexec.rs:80` pins the pgrep line's EXISTENCE as the
  `driver_check` anchor in a positional assertion (`loop_top < compare <
  exec < driver_check && exec < build` — the T50 re-exec block must stay
  the first statement in the while body). That anchor must be updated to
  the new line; the positional assertion must survive.
- LOOP-SPEC's orchestrator-level single-driver hard rule is out of scope
  (it is a model-run `ps` inspection; `ps` works — verified).

## Requirements

1. Replace the pgrep condition with a ps-based equivalent:
   `ps -ax -o command= | grep -q "[c]hug run --spec LOOP-SPEC.md"`.
   The `[c]hug` bracket idiom is REQUIRED — it excludes the grep process's
   own argv from matching. The supervisor's own argv
   (`bash /Users/jadams/workspace/chug/loopd.sh`) does not contain the
   needle, so no self-match; `chug chat` does not match by design.
2. Behavior on match is UNCHANGED: same log line
   (`another LOOP-SPEC driver active; skipping` — operators may grep it),
   same `sleep 120`, same `continue`. Only the detection changes.
3. Replace the comment above the line with one that records WHY ps and
   not pgrep: pgrep persistently fails to enumerate the launchd-spawned
   loopd tree on macOS (pgrep -f/-l/-P all miss; ps sees), so a
   pgrep-based guard fails open; cite "cycle-24 eval I1".
4. Update `tests/loopd_reexec.rs`:
   - the `driver_check` anchor (line ~80) must find the NEW ps-based line
     (match a stable substring, e.g. `ps -ax -o command=` and/or the
     `[c]hug run --spec LOOP-SPEC.md` needle);
   - the positional assertion (`loop_top < compare < exec < driver_check
     && exec < build`) is preserved verbatim in meaning;
   - ADD a pin that the pgrep form is GONE: `loopd.sh` must NOT contain
     `pgrep -f "chug run --spec LOOP-SPEC.md"` (a revert fails the suite);
   - ADD a pin that the `[c]hug` bracket idiom is present in the check
     (dropping the bracket re-introduces grep self-matching).
5. `bash -n loopd.sh` clean.

## Tests

The `tests/loopd_reexec.rs` changes above ARE the tests (static pins —
this file's established pattern for supervisor-shape assertions). No new
test file. Non-vacuousness: deleting the ps line or restoring pgrep must
each fail at least one pin (verify by hand once during implementation,
then revert).

## Acceptance

- `bash -n loopd.sh` clean; `cargo test` green in the worktree.
- `grep -n 'pgrep -f "chug run --spec LOOP-SPEC.md"' loopd.sh` → no match.
- `grep -n '\[c\]hug run --spec LOOP-SPEC.md' loopd.sh` → exactly one match
  inside the single-driver `if`.
- The skip behavior (log line, sleep, continue) is byte-identical.
- Manual smoke (optional, cheap): while a real `chug run --spec
  LOOP-SPEC.md` is live, run the new pipeline by hand — it exits 0
  (match); run it when none is live — exits 1.

check: bash -n loopd.sh && ! grep -q 'pgrep -f "chug run --spec LOOP-SPEC.md"' loopd.sh && grep -q 'ps -ax -o command=' loopd.sh && grep -q '\[c\]hug run --spec LOOP-SPEC.md' loopd.sh && cargo test --test loopd_reexec
