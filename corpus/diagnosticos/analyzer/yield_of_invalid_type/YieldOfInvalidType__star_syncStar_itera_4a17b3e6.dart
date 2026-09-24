Iterable<int> f() sync* {
  yield* g();
//       ^^^
// [diag.yieldEachOfInvalidType] The type 'Iterable<dynamic>' implied by the 'yield*' expression must be assignable to 'Iterable<int>'.
}

Iterable g() => throw 0;
