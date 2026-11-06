# P001 — vampflake go-live

- **User-visible value**: the sandbox-deploy dogfood pipeline (vampflake) goes from parked to live — first real consumer of the chug-built deploy path.
- **First slice**: clone central-loop on K7, verify gh auth, one manual sandbox-deploy walkthrough (operator-assisted, 3 steps total).
- **Estimate class**: S (hours, mostly operator presence).
- **Source fact**: parked since 2026-09; the only blockers ever named are the three operator steps, no code gaps.
