class C<T> {
  final T x = y;
//            ^
// [context 1] The exception is 'In a const constructor, a value of type 'int' can't be assigned to the field 'x', which has type 'String'.' and occurs here.
// [diag.invalidAssignment] A value of type 'int' can't be assigned to a variable of type 'T'.
  const C();
}
const int y = 1;
var v = const C<String>();
//      ^^^^^^^^^^^^^^^^^
// [diag.constEvalThrowsException][context 1] Evaluation of this constant expression throws an exception.
