Iterable<int> f() sync* {
  yield* g();
//       ^^^
// [diag.yieldEachOfInvalidType] The type 'Iterable<String>' implied by the 'yield*' expression must be assignable to 'Iterable<int>'.
}

Iterable<String> g() => throw 0;
