void f(int a) {
  [if (false) (a) = 0];
//            ^^^^^^^
// [diag.deadCode] Dead code.
}
