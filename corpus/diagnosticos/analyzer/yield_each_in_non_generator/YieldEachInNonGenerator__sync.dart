f() {
  yield* 0;
//^^^^^^^^^
// [diag.yieldEachInNonGenerator] Yield-each statements must be in a generator function (one marked with either 'async*' or 'sync*').
}
