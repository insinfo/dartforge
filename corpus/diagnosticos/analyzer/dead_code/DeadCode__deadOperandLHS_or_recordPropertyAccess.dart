void f(({bool b, }) r) {
  if (true || r.b) {}
//         ^^^^^^
// [diag.deadCode] Dead code.
}
