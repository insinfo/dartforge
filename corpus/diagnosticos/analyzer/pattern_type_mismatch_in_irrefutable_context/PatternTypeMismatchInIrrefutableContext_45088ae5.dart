void f(int Function(int) a, (A,) x) {
  (a) = x;
// ^
// [diag.patternTypeMismatchInIrrefutableContext] The matched value of type '(A,)' isn't assignable to the required type 'int Function(int)'.
}

class A {
  int call(int x) => x;
}
