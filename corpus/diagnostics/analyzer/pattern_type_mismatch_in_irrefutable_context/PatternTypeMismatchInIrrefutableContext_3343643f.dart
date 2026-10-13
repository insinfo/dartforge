void f(Object x) {
  var String(length: a) = x;
//    ^^^^^^^^^^^^^^^^^
// [diag.patternTypeMismatchInIrrefutableContext] The matched value of type 'Object' isn't assignable to the required type 'String'.
//                   ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
