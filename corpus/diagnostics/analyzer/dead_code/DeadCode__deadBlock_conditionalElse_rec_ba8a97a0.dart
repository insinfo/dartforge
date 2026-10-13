void f(({int x, int y}) p) {
  true ? p.x : p.y;
//             ^^^
// [diag.deadCode] Dead code.
}
