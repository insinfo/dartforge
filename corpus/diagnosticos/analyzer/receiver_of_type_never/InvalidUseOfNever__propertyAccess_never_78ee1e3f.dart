typedef N = Never;
void f(N x) {
  x.foo;
//  ^^^^
// [diag.deadCode] Dead code.
}
