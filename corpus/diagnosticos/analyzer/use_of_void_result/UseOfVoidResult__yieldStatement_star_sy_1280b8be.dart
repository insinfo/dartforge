Object? f(void x) sync* {
  yield* x;
//       ^
// [diag.uncheckedUseOfNullableValueInYieldEach] A nullable expression can't be used in a yield-each statement.
// [diag.yieldEachOfInvalidType] The type 'void' implied by the 'yield*' expression must be assignable to 'Object'.
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
