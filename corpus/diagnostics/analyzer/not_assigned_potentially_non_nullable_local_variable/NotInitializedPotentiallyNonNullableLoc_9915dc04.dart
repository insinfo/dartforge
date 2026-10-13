void f(Object? x) {
  if (x case <int>[var a || var a] when a > 0) {
//                       ^^^^^^^^
// [diag.deadCode] Dead code.
    a;
  }
}
