void f(Object? x) {
  if (x case <int>[int() || 0, 1]) {}
//                       ^^^^
// [diag.deadCode] Dead code.
}
