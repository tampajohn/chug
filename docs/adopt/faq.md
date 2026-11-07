# FAQ

**What does this cost?**

The public numbers are on the site: the cost tile on the
[home page](https://chug.sh/) and the per-day table on the
[docs page](https://chug.sh/docs.html#cost) — daily spend, baked in at
every sync. The loop's own cost boundary: empty days take a $0 mechanical
cheap exit, a drained queue goes dormant (a weekly read-only audit, not
manufactured work), and routine cycles ride the cheaper routine model
unless the day's freshness rule escalates.

**Is it safe to point at my repo?**

The trust boundary is mechanical, not hopeful. Every shell call passes
the risk gate — destructive calls are blocked with an error the model can
route around — and `.chug/hooks.json` can veto any tool call, while
`.chug/permissions.json` deny-rules fail closed. Tools are sandboxed to
the run cwd, and the goal gate is the spec's `check:` re-running, never a
model's claim. The README's [risk gate, hooks, and permissions
sections](https://github.com/tampajohn/chug/blob/main/README.md) name
every knob.

**Do I need an Anthropic account?**

No. chug speaks the Anthropic Messages wire protocol; point
`ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN` at any provider that speaks
it. The repo's own loop runs kimi for judgment calls and glm for routine
phases — per-phase routing is a mechanical rule in the supervisor
(`./loopd.sh routing` prints what the next cycle would get), never a
model's choice.

**How do I uninstall?**

`./loopd.sh stop` (and remove the launchd plist, if you installed one),
delete the binary — `rm ~/.local/bin/chug`, or `cargo uninstall chug` if
you installed from source — and delete any per-repo state you do not want
(the `.chug/` directory inside each repo it ran in). Nothing else was
installed.
