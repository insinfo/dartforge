void f(Object? x) {
  switch (x) {
    case [int a,]:
//            ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
    case (int a,) when a > 0:
      break;
  };
}
