f() async {
  yield 0;
//^^^^^^^^
// [diag.yieldInNonGenerator] Yield statements must be in a generator function (one marked with either 'async*' or 'sync*').
}
