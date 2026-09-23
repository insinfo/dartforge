void f(Object x) {
  if (x case Null _) {}
//           ^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'Object' can never match the required type 'Null'.
//                   ^^
// [diag.deadCode] Dead code.
}
