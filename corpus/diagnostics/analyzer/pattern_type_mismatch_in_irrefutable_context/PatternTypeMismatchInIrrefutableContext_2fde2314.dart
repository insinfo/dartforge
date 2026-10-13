void f(Object x) {
  var <int, String>{0: a} = x;
//    ^^^^^^^^^^^^^^^^^^^
// [diag.patternTypeMismatchInIrrefutableContext] The matched value of type 'Object' isn't assignable to the required type 'Map<int, String>'.
//                     ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
