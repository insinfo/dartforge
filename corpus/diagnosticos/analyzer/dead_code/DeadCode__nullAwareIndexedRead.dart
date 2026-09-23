void f(Null n, int i) {
  n?[i];
//   ^^
// [diag.deadCode] Dead code.
  print('reached');
}
