The green suite misses several real failures, including policy bypass, false completion, broken crash recovery, and a nonexclusive driver lock.

This is a source-level review. I did not run the Rust suite because the workspace is read-only. I confirmed the signal-exit scenario with a harmless subprocess probe. No files were changed.

## 1. CORRECTNESS BUGS

### HIGH — Denied `goal_complete` still completes the run

**Evidence:** [src/driver.rs:967](/Users/jadams/workspace/chug/src/driver.rs:967), [src/driver.rs:1078](/Users/jadams/workspace/chug/src/driver.rs:1078), [src/driver.rs:1164](/Users/jadams/workspace/chug/src/driver.rs:1164).

**Trigger:** Configure `{"permissions":{"deny":[{"tool":"goal_complete"}]}}`, then have the model call `goal_complete` with no check command, or with `check: true`.

The permission gate produces an error, but `goal_summary` is populated solely from the tool name. Verification then accepts and exits successfully. A PreToolUse veto has the same failure. Unlike `submit_plan`, this path never checks `result.is_error`.

### HIGH — Truncated SSE responses are accepted, including unfinished tool calls

**Evidence:** [src/api.rs:817](/Users/jadams/workspace/chug/src/api.rs:817), [src/api.rs:784](/Users/jadams/workspace/chug/src/api.rs:784), [src/api.rs:976](/Users/jadams/workspace/chug/src/api.rs:976).

**Trigger:** A proxy returns HTTP 200 SSE containing a tool block and syntactically complete input JSON, then ends the HTTP body before `content_block_stop` or `message_stop`.

`finish()` closes the open block and returns success. `message_stop` is ignored rather than tracked. The driver can execute the partial response’s tool call. If the stream ends immediately after a tool block starts, missing input becomes `{}`; an unfinished `goal_complete` can consequently terminate a run without a check.

A clean HTTP EOF is enough; no transport error is required.

### HIGH — Two concurrent starters can both acquire `driver.lock`

**Evidence:** [src/driver_lock.rs:189](/Users/jadams/workspace/chug/src/driver_lock.rs:189), [src/driver_lock.rs:206](/Users/jadams/workspace/chug/src/driver_lock.rs:206), [src/driver_lock.rs:238](/Users/jadams/workspace/chug/src/driver_lock.rs:238).

**Trigger/interleaving:**

1. A and B both read an absent or stale lock.
2. A writes its PID, verifies it, and returns a guard.
3. B writes its PID, verifies it, and returns a guard.

Both proceed into transcript rotation and writes. Read-after-write verification does not make acquisition atomic. The explicit lockless fallback creates additional failure-open paths, but this race occurs even with a healthy filesystem and working process probes.

### HIGH — A crash during tool execution leaves an unresumable conversation

**Evidence:** [src/driver.rs:930](/Users/jadams/workspace/chug/src/driver.rs:930), [src/driver.rs:943](/Users/jadams/workspace/chug/src/driver.rs:943), [src/driver.rs:1208](/Users/jadams/workspace/chug/src/driver.rs:1208), [src/driver.rs:540](/Users/jadams/workspace/chug/src/driver.rs:540).

**Trigger:** Kill chug after an assistant message containing `tool_use` is appended, while its bash command is running.

Tool results are persisted only after the entire batch finishes. Resume loads the unanswered tool calls without repairing them. The next request can contain missing `tool_result` blocks and be rejected by the endpoint. Earlier tools in the same batch may already have changed files, with no persisted result recording that fact.

Separately, [src/transcript.rs:77](/Users/jadams/workspace/chug/src/transcript.rs:77) rewrites the live transcript using a truncating write. A crash or write failure during trimming can destroy the only active transcript; malformed trailing JSON also aborts loading at [src/transcript.rs:59](/Users/jadams/workspace/chug/src/transcript.rs:59).

### HIGH — Model text can spoof supervisor success

**Evidence:** [loopd.sh:225](/Users/jadams/workspace/chug/loopd.sh:225), [src/events.rs:264](/Users/jadams/workspace/chug/src/events.rs:264).

**Trigger:** The model says “Look for `chug: goal complete` in the log,” then the run fails verification or exhausts its budget.

Raw model text reaches the cycle log. The supervisor searches that mixed-content log for the substring and ignores the child’s exit status. It records `cycle OK`, resets consecutive failures, and runs site sync despite no accepted goal.

### HIGH — Failed builds do not prevent running an old binary

**Evidence:** [loopd.sh:15](/Users/jadams/workspace/chug/loopd.sh:15), [loopd.sh:182](/Users/jadams/workspace/chug/loopd.sh:182), [loopd.sh:221](/Users/jadams/workspace/chug/loopd.sh:221).

**Trigger:** A merged change fails compilation while a previous `target/release/chug` exists.

The script uses `set -u`, does not test the build’s status, and launches the previous executable anyway. An inherited `CARGO_TARGET_DIR` can also send a successful build elsewhere while the supervisor still launches the fixed `./target/release/chug` path.

### MEDIUM — Signal-killed shell commands are reported as successful

**Evidence:** [src/tools.rs:860](/Users/jadams/workspace/chug/src/tools.rs:860), [src/tools.rs:584](/Users/jadams/workspace/chug/src/tools.rs:584).

**Trigger:** `bash {"command":"kill -TERM $$"}`, or an executed process dies from a signal.

`ExitStatus::code()` becomes `None`, while `timed_out` remains false. The error predicate evaluates false. The result simultaneously reports success and labels the exit “none (killed after timeout),” although no timeout occurred. My subprocess probe confirmed signal termination for this command.

### MEDIUM — MCP deadlines leave permanently blocked workers and sockets

**Evidence:** [src/mcp_http.rs:158](/Users/jadams/workspace/chug/src/mcp_http.rs:158), [src/mcp_http.rs:493](/Users/jadams/workspace/chug/src/mcp_http.rs:493), [src/mcp_http.rs:637](/Users/jadams/workspace/chug/src/mcp_http.rs:637).

**Trigger:** An MCP server sends response headers, then leaves the body open without another newline or EOF.

The caller times out and sets `cancel`, but the worker checks cancellation only before its blocking `read_line`. The client has no total timeout. Repeated calls accumulate blocked threads and open connections; dropping the server intentionally abandons those workers at [src/mcp_http.rs:1012](/Users/jadams/workspace/chug/src/mcp_http.rs:1012).

### MEDIUM — Output truncation does not bound memory consumption

**Evidence:** [src/tools.rs:843](/Users/jadams/workspace/chug/src/tools.rs:843), [src/tools.rs:583](/Users/jadams/workspace/chug/src/tools.rs:583).

**Trigger:** Run `yes`, or a build command that emits gigabytes before its timeout.

Both pipes accumulate into unbounded vectors with `read_to_end`. The 30,000-character cap is applied only after execution returns, so the harness can exhaust memory before the cap or timeout helps. Text `read_file` likewise reads the entire file before pagination at [src/tools.rs:312](/Users/jadams/workspace/chug/src/tools.rs:312).

## 2. SECURITY ISSUES

### HIGH — The tools are not confined to cwd; symlinks bypass even the file-tool checks

**Evidence:** [src/tools.rs:759](/Users/jadams/workspace/chug/src/tools.rs:759), [src/tools.rs:471](/Users/jadams/workspace/chug/src/tools.rs:471), [src/tools.rs:817](/Users/jadams/workspace/chug/src/tools.rs:817), [src/plan.rs:141](/Users/jadams/workspace/chug/src/plan.rs:141).

**Trigger:** A checkout contains `outside -> /some/external/directory`. Call `read_file`, `write_file`, or `edit_file` on `outside/file`.

`resolve_safe` validates lexical components only; filesystem operations follow symlinks. The same escape applies to plan-mode reads and `submit_plan --out outside/file`. No race is necessary.

Bash has no filesystem sandbox at all: `current_dir(cwd)` merely chooses its initial directory. It can access absolute paths and inherits the process environment, including supplied API credentials. A model-requested `env` can expose those secrets to subsequent API requests and the transcript; result content is copied verbatim at [src/driver.rs:1105](/Users/jadams/workspace/chug/src/driver.rs:1105).

### HIGH — Repository-controlled `mcp.json` executes code before permission enforcement

**Evidence:** [src/mcp.rs:366](/Users/jadams/workspace/chug/src/mcp.rs:366), [src/mcp.rs:469](/Users/jadams/workspace/chug/src/mcp.rs:469), [src/driver.rs:295](/Users/jadams/workspace/chug/src/driver.rs:295), [src/driver.rs:658](/Users/jadams/workspace/chug/src/driver.rs:658).

**Trigger:** Start a normal run in an untrusted checkout containing an MCP entry whose command is `sh` with attacker-controlled arguments.

The local configuration is discovered automatically and its process is spawned before permissions load. Denying `bash` or `mcp__*` does not prevent this startup execution. The process inherits credentials unless explicitly overridden. A failed MCP handshake occurs after the command has already executed.

### HIGH — Editable spec checks bypass bash permissions and the risk gate

**Evidence:** [src/driver.rs:822](/Users/jadams/workspace/chug/src/driver.rs:822), [src/driver.rs:1390](/Users/jadams/workspace/chug/src/driver.rs:1390).

**Trigger:** Deny the bash tool but permit file editing. The model changes its spec to contain an arbitrary `check:` shell command, then calls `goal_complete` on the following iteration.

Verification invokes `run_shell` directly, outside permissions, hooks, and the risk gate. This permits shell execution despite the bash deny. Replacing a failing check with `check: true` also defeats verification integrity.

`edit_file` itself performs literal string replacement; the injection occurs when the edited text becomes an executable spec check.

## 3. DOCTRINE-VS-CODE DRIFT

### HIGH — “All paths sandboxed” is false

**Claim:** [README.md:337](/Users/jadams/workspace/chug/README.md:337) names exactly two exceptions; [SPEC.md:19](/Users/jadams/workspace/chug/SPEC.md:19) explicitly includes bash in confinement.

**Counterexample:** The symlink and absolute-path bash triggers above. [src/tools.rs:755](/Users/jadams/workspace/chug/src/tools.rs:755) explicitly documents no symlink resolution, while [src/tools.rs:753](/Users/jadams/workspace/chug/src/tools.rs:753) recommends bash for cross-tree access. This is a conflicting security contract, not merely missing documentation.

### MEDIUM — Final automatic verification bypasses the mandated main-only target cache

**Claim:** [LOOP-SPEC.md:457](/Users/jadams/workspace/chug/LOOP-SPEC.md:457) requires final main-tree gates to use `target-shared-main`.

**Actual path:** [loopd.sh:221](/Users/jadams/workspace/chug/loopd.sh:221) launches the driver with `target-shared`; [LOOP-SPEC.md:9](/Users/jadams/workspace/chug/LOOP-SPEC.md:9) specifies plain `cargo test`; [src/driver.rs:1390](/Users/jadams/workspace/chug/src/driver.rs:1390) executes it with the inherited environment.

**Trigger:** Finish a normal supervisor cycle. The automatic acceptance check runs against the shared cache regardless of whether the model previously ran compliant manual gates, reintroducing the cache-sharing condition the doctrine explicitly forbids.

### MEDIUM — MCP’s advertised total timeout is per attempt, and shutdown does not close streams

**Claim:** [README.md:569](/Users/jadams/workspace/chug/README.md:569) promises a 60-second per-call total; [specs/spec-9-mcp-http.md:61](/Users/jadams/workspace/chug/specs/spec-9-mcp-http.md:61) requires closing streams at shutdown.

**Evidence:** [src/mcp_http.rs:305](/Users/jadams/workspace/chug/src/mcp_http.rs:305), [src/mcp_http.rs:358](/Users/jadams/workspace/chug/src/mcp_http.rs:358), [src/mcp_http.rs:1012](/Users/jadams/workspace/chug/src/mcp_http.rs:1012).

**Trigger:** Three slow connection failures followed by a stalled successful connection. Every retry receives a fresh deadline, plus backoff, allowing the call to exceed 60 seconds. A silent stream also remains open after shutdown, as described above.

## 4. TEST GAPS — INCLUDING VACUOUS COVERAGE

### HIGH — The “mid-stream failure” test never delivers a partial first stream

**Evidence:** [src/api.rs:2692](/Users/jadams/workspace/chug/src/api.rs:2692), [src/api.rs:2702](/Users/jadams/workspace/chug/src/api.rs:2702), [src/api.rs:2324](/Users/jadams/workspace/chug/src/api.rs:2324).

The comment claims attempt one fails **after a chunk was fed**. Its fixture supplies only `Err(Connection(...))`; the fake’s error branch returns without invoking `on_chunk`.

**Escaping regression:** Preserve a previous attempt’s accumulated blocks across retries. This test still passes because the first accumulator was never populated. Its partial-stream-discard coverage is vacuous, although its retry-count assertion is useful. It also misses the clean-EOF failure in finding 2.

### HIGH — Driver policy tests omit denied termination tools

**Evidence:** [src/driver/tests/permissions_policy.rs:43](/Users/jadams/workspace/chug/src/driver/tests/permissions_policy.rs:43), [src/driver/tests/permissions_policy.rs:115](/Users/jadams/workspace/chug/src/driver/tests/permissions_policy.rs:115).

These exercise denied bash execution and hook ordering.

**Missing trigger:** Deny or veto `goal_complete`, use no check or `check: true`, and assert that no `GoalAccepted` event occurs and the next model iteration runs. That is the exact uncovered path responsible for finding 1.

### HIGH — Lock tests exercise sequential acquisition, not simultaneous ownership

**Evidence:** [src/driver/tests/lock.rs:71](/Users/jadams/workspace/chug/src/driver/tests/lock.rs:71).

The successor starts after the first run has exited.

**Missing trigger:** Synchronize two processes after both read an absent lock, then allow each to write and verify. Assert that only one enters transcript housekeeping. Sequential successor tests cannot expose finding 3.

### MEDIUM — MCP shutdown tests measure return latency, not resource release

**Evidence:** [src/mcp_http.rs:2265](/Users/jadams/workspace/chug/src/mcp_http.rs:2265), [src/mcp_http.rs:2395](/Users/jadams/workspace/chug/src/mcp_http.rs:2395).

The tests release the stalled server themselves after checking that the caller returned.

**Missing trigger:** Keep the server silent after timeout/drop and require the peer to observe connection closure. Repeat to detect accumulating workers. The current tests pass with the resource leak intact.

### MEDIUM — Tool safety tests cover lexical traversal, not actual filesystem confinement

**Evidence:** [src/tools.rs:1009](/Users/jadams/workspace/chug/src/tools.rs:1009).

The path tests exercise `..` and absolute-path strings.

**Missing triggers:** An in-tree symlink to an external fixture; a signal-killed shell; output exceeding a bounded capture buffer. These expose the confinement, false-success, and memory failures above.

### MEDIUM — Trim tests do not establish crash-safe persistence

**Evidence:** [src/trim.rs:311](/Users/jadams/workspace/chug/src/trim.rs:311), [src/transcript.rs:68](/Users/jadams/workspace/chug/src/transcript.rs:68).

In-memory prefix preservation is tested; interruption of the persisted rewrite is not.

**Missing trigger:** Fail or terminate a rewrite after the existing transcript is truncated but before the replacement completes, then resume. The essential assertion is recovery of a complete old or new transcript.

### LOW — A trim assertion is tautological

**Evidence:** [src/trim.rs:257](/Users/jadams/workspace/chug/src/trim.rs:257), [src/trim.rs:45](/Users/jadams/workspace/chug/src/trim.rs:45).

Inside `if is_trim_marker(msg)`, the test asserts `msg.role == "user"`. The predicate already requires exactly that.

**Escaping regression:** Emit assistant-role marker text. This assertion is skipped rather than failing. Other assertions may catch the change; this particular assertion contributes no independent coverage.

The documentation string tests are not wholly vacuous—they can detect wording changes. But [tests/loopd_reexec.rs:94](/Users/jadams/workspace/chug/tests/loopd_reexec.rs:94) checks source-string ordering, not supervisor behavior. A child that prints the success substring and exits nonzero, or a failed build followed by a stale executable, remains outside that coverage.
