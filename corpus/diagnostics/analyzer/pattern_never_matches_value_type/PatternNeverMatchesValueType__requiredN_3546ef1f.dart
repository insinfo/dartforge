void f(void Function() x) {
  if (x case Null _) {}
//           ^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'void Function()' can never match the required type 'Null'.
//                   ^^
// [diag.deadCode] Dead code.
}
