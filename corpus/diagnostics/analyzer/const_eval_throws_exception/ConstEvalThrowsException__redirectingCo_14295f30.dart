class A {
  const A.a1(x) : this.a2(x);
//                        ^
// [context 1] The exception is 'A value of type 'int' can't be assigned to a parameter of type 'String' in a const constructor.' and occurs here.
  const A.a2(String x);
}
var v = const A.a1(0);
//      ^^^^^^^^^^^^^
// [diag.constEvalThrowsException][context 1] Evaluation of this constant expression throws an exception.
