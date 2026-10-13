void f<T extends num>(T x) {
  if (x case Null _) {}
//           ^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'T' can never match the required type 'Null'.
//                   ^^
// [diag.deadCode] Dead code.
}
