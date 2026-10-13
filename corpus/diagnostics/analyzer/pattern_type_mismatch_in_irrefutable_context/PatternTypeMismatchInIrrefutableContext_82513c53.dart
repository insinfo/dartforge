void f(int a, num x) {
  (a) = x;
// ^
// [diag.patternTypeMismatchInIrrefutableContext] The matched value of type 'num' isn't assignable to the required type 'int'.
}
