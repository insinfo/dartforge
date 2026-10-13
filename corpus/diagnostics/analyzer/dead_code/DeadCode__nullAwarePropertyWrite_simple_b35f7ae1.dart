void f(Null n, int i) {
  n?.p = i;
//   ^^^^^
// [diag.deadCode] Dead code.
  print('reached');
}
