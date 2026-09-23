void f(Object? x) {
  switch (x) {
    case 0:
    case [var a]:
      a;
//    ^
// [diag.patternVariableSharedCaseScopeNotAllCases] The variable 'a' is available in some, but not all cases that share this body.
  };
}
