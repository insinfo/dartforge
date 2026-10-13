class A {
  Never get foo => throw 0;
}

void f(Object x) {
  if (x case A(foo: _)) {}
//                      ^^
// [diag.deadCode] Dead code.
}
