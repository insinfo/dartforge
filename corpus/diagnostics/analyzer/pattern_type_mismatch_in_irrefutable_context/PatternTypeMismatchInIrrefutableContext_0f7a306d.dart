void f(num x) {
  var (int a) = x;
//     ^^^^^
// [diag.patternTypeMismatchInIrrefutableContext] The matched value of type 'num' isn't assignable to the required type 'int'.
//         ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
