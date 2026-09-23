void f(int x) {
  switch (x) {
    case final a:
//             ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
      switch (x) {
        case 2:
          return;
      }
      return;
  }
}
