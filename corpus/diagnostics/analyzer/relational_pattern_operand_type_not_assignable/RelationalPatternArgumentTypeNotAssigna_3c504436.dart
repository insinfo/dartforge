class A {}

void f(A x) {
  switch (x) {
    case == null:
      break;
//    ^^^^^^
// [diag.deadCode] Dead code.
  }
}
