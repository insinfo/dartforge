typedef N = Never;
void f(N x) {
  x.hashCode;
//  ^^^^^^^^^
// [diag.deadCode] Dead code.
}
