void f((A,) x) {
  var (int Function(int) v,) = x;
//     ^^^^^^^^^^^^^^^^^^^
// [diag.patternTypeMismatchInIrrefutableContext] The matched value of type 'A' isn't assignable to the required type 'int Function(int)'.
//                       ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
}

class A {
  int call(int x) => x;
}
