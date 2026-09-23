f() async* {
  yield* 0;
//       ^
// [diag.yieldEachOfInvalidType] The type 'int' implied by the 'yield*' expression must be assignable to 'Stream<dynamic>'.
}
