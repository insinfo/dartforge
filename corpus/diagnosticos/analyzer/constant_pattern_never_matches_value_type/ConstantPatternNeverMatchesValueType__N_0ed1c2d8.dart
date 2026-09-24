void f((int, int) x) {
  if (x case null) {}
//           ^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type '(int, int)' can never be equal to this constant of type 'Null'.
//                 ^^
// [diag.deadCode] Dead code.
}
