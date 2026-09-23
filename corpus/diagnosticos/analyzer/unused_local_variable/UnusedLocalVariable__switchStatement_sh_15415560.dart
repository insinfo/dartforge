void f(Object? x) {
  switch (x) {
    case 0:
    case [var a]:
//            ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
      break;
  };
}
