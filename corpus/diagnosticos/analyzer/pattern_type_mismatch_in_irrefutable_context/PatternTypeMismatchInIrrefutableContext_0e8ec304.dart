void f(Object x) {
  var [a] = x;
//    ^^^
// [diag.patternTypeMismatchInIrrefutableContext] The matched value of type 'Object' isn't assignable to the required type 'List<Object?>'.
//     ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
