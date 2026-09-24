import 'dart:math';
// T max<T extends num>(T x, T y);
main() {
  num x;
  dynamic y;

  num a = max(x, y);
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//            ^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'x' must be assigned before it can be used.
  Object b = max(x, y);
//       ^
// [diag.unusedLocalVariable] The value of the local variable 'b' isn't used.
//               ^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'x' must be assigned before it can be used.
  dynamic c = max(x, y);
//        ^
// [diag.unusedLocalVariable] The value of the local variable 'c' isn't used.
//                ^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'x' must be assigned before it can be used.
  var d = max(x, y);
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'd' isn't used.
//            ^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'x' must be assigned before it can be used.
}
