void f(int x) {
  if (x case (true)) {}
//            ^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'int' can never be equal to this constant of type 'bool'.
}
