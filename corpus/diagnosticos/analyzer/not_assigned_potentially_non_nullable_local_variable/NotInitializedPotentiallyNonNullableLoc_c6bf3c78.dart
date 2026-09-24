void f(Object? x) {
  switch (x) {
    case <int>[var a || var a] when a > 0:
//                   ^^^^^^^^
// [diag.deadCode] Dead code.
      a;
  }
}
