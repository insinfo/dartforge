void f(Null n, int i) {
  n?.foo(i);
//   ^^^^^^
// [diag.deadCode] Dead code.
  print('reached');
}
