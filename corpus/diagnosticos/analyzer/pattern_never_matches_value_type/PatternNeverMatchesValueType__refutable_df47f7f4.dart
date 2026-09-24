void f((int,) x) {
  switch (x) {
    case (int f1, int f2):
//       ^^^^^^^^^^^^^^^^
// [diag.patternNeverMatchesValueType] The matched value type '(int,)' can never match the required type '(Object?, Object?)'.
//            ^^
// [diag.unusedLocalVariable] The value of the local variable 'f1' isn't used.
//                    ^^
// [diag.unusedLocalVariable] The value of the local variable 'f2' isn't used.
      break;
  }
}
