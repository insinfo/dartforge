void f<T extends Object>(T x) {
  if (x case null) {}
//           ^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'T' can never be equal to this constant of type 'Null'.
//                 ^^
// [diag.deadCode] Dead code.
}
