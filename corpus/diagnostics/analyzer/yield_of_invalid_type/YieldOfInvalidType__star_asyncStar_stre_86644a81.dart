Stream<int> f() async* {
  yield* g();
//       ^^^
// [diag.yieldEachOfInvalidType] The type 'Stream<String>' implied by the 'yield*' expression must be assignable to 'Stream<int>'.
}

Stream<String> g() => throw 0;
