void f(Never x) {
  x.foo = 0;
//        ^^
// [diag.deadCode] Dead code.
}
