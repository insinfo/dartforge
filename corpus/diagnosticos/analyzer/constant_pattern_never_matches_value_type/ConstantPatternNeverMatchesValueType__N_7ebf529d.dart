void f(void Function() x) {
  if (x case null) {}
//           ^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'void Function()' can never be equal to this constant of type 'Null'.
//                 ^^
// [diag.deadCode] Dead code.
}
