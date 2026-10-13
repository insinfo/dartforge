const int minValue = -9223372036854775808;
const int zero = minValue - minValue;
const int result = 1 ~/ zero;
//                 ^^^^^^^^^
// [diag.constEvalThrowsIdbze] Evaluation of this constant expression throws an IntegerDivisionByZeroException.
