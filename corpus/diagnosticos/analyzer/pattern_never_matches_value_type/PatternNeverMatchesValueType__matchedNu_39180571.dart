void f(Null x) {
  if (x case Object _) {}
//           ^^^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'Null' can never match the required type 'Object'.
//                     ^^
// [diag.deadCode] Dead code.
}
