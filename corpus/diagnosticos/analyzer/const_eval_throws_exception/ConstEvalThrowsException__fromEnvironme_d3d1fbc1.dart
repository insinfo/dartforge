var b1 = const bool.fromEnvironment(1);
//       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.constEvalThrowsException] Evaluation of this constant expression throws an exception.
//                                  ^
// [diag.argumentTypeNotAssignable] The argument type 'int' can't be assigned to the parameter type 'String'.
var b2 = const bool.fromEnvironment('x', defaultValue: 1);
//       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.constEvalThrowsException] Evaluation of this constant expression throws an exception.
//                                                     ^
// [diag.argumentTypeNotAssignable] The argument type 'int' can't be assigned to the parameter type 'bool'.
