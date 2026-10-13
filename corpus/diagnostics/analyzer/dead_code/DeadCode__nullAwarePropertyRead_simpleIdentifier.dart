void f(Null n) {
  n?.p;
//   ^
// [diag.deadCode] Dead code.
  print('reached');
}
