void f(({int foo}) x) {
  var (a,) = x;
//    ^^^^
// [diag.patternTypeMismatchInIrrefutableContext] The matched value of type '({int foo})' isn't assignable to the required type '(Object?,)'.
//     ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
