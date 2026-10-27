# Decisions: changes we make on purpose

Baseline: the JS prototype in `reference/js/` (TÜDAF rules, one-ply agent with the tuned U). Git tag: `baseline`.
Measure every change against the baseline with fixed seeds, so each number in the talk has a source.

| Date | Change | Why | How we measured | Result |
|---|---|---|---|---|
| 2026-10-27 | Baseline (tag `baseline`, 5dbcd00): the Rust port of the prototype, one-ply agent with the tuned U (1/2.5/-0.05) | The reference point for every later change: all three referees green, and arena games identical move for move to `playGame` in reference/js/agent.js (6000 games checked) | `dama arena`, release build, 2000 games per pairing, seed 1, colours alternate, 2 random opening plies; rows 1-4 of `results/arena.csv` | Tuned's score (± SE): vs random 97.17% ± 0.27; vs material 97.08% ± 0.28 (talk: 97.2%); vs first 99.52% ± 0.11 (talk: 99.6%). First vs material: 4.17% ± 0.45 |
