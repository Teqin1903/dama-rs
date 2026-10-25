// Times perft of reference/js/engine.js from the start position, for depths 1 up to the given
// depth (default 8), with the same method as engine/examples/perft.rs (described there).
//
//   taskset -c 2 node bench/perft.js 8
//
// Output: one tab-separated line per depth: depth, leaves, nanoseconds per run, k, samples.
'use strict';
const D = require('../reference/js/engine.js');

const maxDepth = Number(process.argv[2] || 8);
const board = D.initial();

// Runs perft k times; returns [nanoseconds as a BigInt, leaf count].
function sample(depth, k) {
  let leaves = 0;
  const start = process.hrtime.bigint();
  for (let i = 0; i < k; i++) leaves = D.perft(board, 1, depth);
  return [process.hrtime.bigint() - start, leaves];
}

sample(5, 1); // warm-up
console.log('depth\tleaves\tns_per_run\tk\tsamples');
for (let depth = 1; depth <= maxDepth; depth++) {
  let k = 1;
  let [first, leaves] = sample(depth, k);
  while (first < 50_000_000n) {
    k *= 2;
    [first, leaves] = sample(depth, k);
  }
  const count = depth <= 6 ? 5 : 3;
  const times = [first];
  while (times.length < count) times.push(sample(depth, k)[0]);
  times.sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  const nsPerRun = times[Math.floor(count / 2)] / BigInt(k);
  console.log(`${depth}\t${leaves}\t${nsPerRun}\t${k}\t${count}`);
}
