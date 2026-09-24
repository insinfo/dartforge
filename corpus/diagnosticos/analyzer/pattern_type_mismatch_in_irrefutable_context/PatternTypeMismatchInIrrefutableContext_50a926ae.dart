void f(int a) {
  (a) = 1.2;
// ^
// [diag.patternTypeMismatchInIrrefutableContext] The matched value of type 'double' isn't assignable to the required type 'int'.
}
