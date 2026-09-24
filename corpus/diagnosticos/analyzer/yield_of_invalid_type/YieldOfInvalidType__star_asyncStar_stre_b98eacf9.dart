Stream<int> f() async* {
  yield* g();
//       ^^^
// [diag.yieldEachOfInvalidType] The type 'Stream<dynamic>' implied by the 'yield*' expression must be assignable to 'Stream<int>'.
}

Stream g() => throw 0;
