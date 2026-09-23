void f<T extends num>(T x) {
  if (x case (true)) {}
//            ^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'T' can never be equal to this constant of type 'bool'.
}
