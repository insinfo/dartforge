void f<T>(T x) {
  if (x is int) {
    if (x case (true)) {}
//              ^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'T & int' can never be equal to this constant of type 'bool'.
  }
}
