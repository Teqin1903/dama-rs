# JS prototype (read-only reference)

- `engine.js`: Turkish draughts rules (TÜDAF). Perft 1-8 from the start equals kish 1.1.2.
- `agent.js`: the baseline agent (one ply, U) and the match runner used in our Assignment 3 experiments.
- `rng.js`: seeded random numbers (mulberry32), so every experiment is reproducible.
- `make_testdata.js`: regenerates `testdata/` from these files: `node reference/js/make_testdata.js`
