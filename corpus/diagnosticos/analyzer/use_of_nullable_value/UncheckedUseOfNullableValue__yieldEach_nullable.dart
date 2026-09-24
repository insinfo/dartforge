m() sync* {
  List<int>? x;
  yield* x;
//       ^
// [diag.uncheckedUseOfNullableValueInYieldEach] A nullable expression can't be used in a yield-each statement.
// [diag.yieldEachOfInvalidType] The type 'List<int>?' implied by the 'yield*' expression must be assignable to 'Iterable<dynamic>'.
}
