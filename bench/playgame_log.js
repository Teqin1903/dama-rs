// Plays a match with playGame from reference/js/agent.js, with the legacy seeds of engine/src/arena.rs,
// and prints every game as `dama arena --log` writes it. Identical output means the Rust arena
// plays the same games as the prototype, move for move.
//
//   node bench/playgame_log.js tuned random 2000 1 > js.tsv
//   cargo run --release -p cli -- arena tuned random --games 2000 --seed 1 --schedule legacy \
//     --csv /tmp/x.csv --log rs.tsv
//   cmp js.tsv rs.tsv
//
// Arguments: agent A, agent B (random, material, first or tuned), games, seed. `--csv` keeps the
// check out of results/arena.csv.
'use strict';
const A = require('../reference/js/agent.js');
const mk = require('../reference/js/rng.js');
const D = A.D;

const WEIGHTS = {
  material: { man: 1, king: 3, mob: 0 },
  first: { man: 1, king: 3, mob: 0.1 },
  tuned: { man: 1, king: 2.5, mob: -0.05 },
};

function policy(name, seed) {
  if (name === 'random') return A.randomPolicy(mk(seed));
  if (!WEIGHTS[name]) throw new Error(`unknown agent ${name}: expected random, material, first or tuned`);
  return A.agentPolicy(WEIGHTS[name], mk(seed));
}

const [a, b] = [process.argv[2], process.argv[3]];
const games = Number(process.argv[4] || 200);
const seed = Number(process.argv[5] || 1);

// One generator per match; game g draws its opening seed, then A's seed, then B's seed.
const seeds = mk(seed);
const nextU32 = () => seeds() * 4294967296;
const lines = ['game\twhite\tblack\tresult\treason\tplies\tmoves'];
for (let g = 0; g < games; g++) {
  const opening = mk(nextU32());
  const pa = policy(a, nextU32());
  const pb = policy(b, nextU32());
  const aWhite = g % 2 === 0;
  const r = A.playGame(aWhite ? pa : pb, aWhite ? pb : pa, {
    randomPlies: 2, rnd: opening, maxPlies: 400, record: true,
  });
  const result = r.result === 1 ? '1-0' : r.result === -1 ? '0-1' : '1/2-1/2';
  const moves = r.rec.map((x) => D.notation(x.m)).join(' ');
  lines.push([g, aWhite ? a : b, aWhite ? b : a, result, r.reason, r.plies, moves].join('\t'));
}
console.log(lines.join('\n'));
