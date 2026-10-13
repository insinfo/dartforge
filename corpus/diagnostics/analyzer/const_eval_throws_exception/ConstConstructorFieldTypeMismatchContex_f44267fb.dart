class A {
  const A(x) : y = x;
//                 ^
// [context 1] The exception is 'In a const constructor, a value of type 'Null' can't be assigned to the field 'y', which has type 'int'.' and occurs here.
  final int y;
}
var v = const A(null);
//      ^^^^^^^^^^^^^
// [diag.constEvalThrowsException][context 1] Evaluation of this constant expression throws an exception.
