void f(Object? x) {
  (switch (x) {
    <int>[var a || var a] when a > 0 => a,
//              ^^^^^^^^
// [diag.deadCode] Dead code.
    _ => 0,
  });
}
