void f(List<bool> x) {
  if (x case (true)) {}
//            ^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'List<bool>' can never be equal to this constant of type 'bool'.
}
