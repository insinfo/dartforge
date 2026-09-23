void f((int,) x) {
  switch (x) {
    case (int f,):
//            ^
// [diag.unusedLocalVariable] The value of the local variable 'f' isn't used.
      break;
  }
}
