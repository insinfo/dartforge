void f(Object? x) {
  switch (x) {
    case 0:
    case [var a]:
//            ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
      a = 1;
//    ^
// [diag.patternVariableSharedCaseScopeNotAllCases] The variable 'a' is available in some, but not all cases that share this body.
  };
}
