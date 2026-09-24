void f(A x) {
  if (x case Null _) {}
//           ^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'A' can never match the required type 'Null'.
//                   ^^
// [diag.deadCode] Dead code.
}

class A {}
