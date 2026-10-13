void f(Null n, int i, int j) {
  n?[i] = j;
//   ^^^^^^
// [diag.deadCode] Dead code.
  print('reached');
}
