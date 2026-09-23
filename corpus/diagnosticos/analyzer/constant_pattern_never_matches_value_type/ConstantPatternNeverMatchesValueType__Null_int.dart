void f(int x) {
  if (x case null) {}
//           ^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'int' can never be equal to this constant of type 'Null'.
//                 ^^
// [diag.deadCode] Dead code.
}
