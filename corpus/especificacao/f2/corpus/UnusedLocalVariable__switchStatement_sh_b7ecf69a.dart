void f(Object? x) {
  switch (x) {
    case (int a,) when a > 0:
    case [int a,]:
//            ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
      break;
  };
}
