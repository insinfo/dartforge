void f(Null x) {
  if (x case A _) {}
//           ^
// [diag.patternNeverMatchesValueType] The matched value type 'Null' can never match the required type 'A'.
//                ^^
// [diag.deadCode] Dead code.
}

class A {}
