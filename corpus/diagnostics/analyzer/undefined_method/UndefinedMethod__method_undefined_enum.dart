enum E { A }
f() => E.abs();
//       ^^^
// [diag.undefinedMethodOnTypeLiteral] The method 'abs' isn't defined for the type 'E'.
