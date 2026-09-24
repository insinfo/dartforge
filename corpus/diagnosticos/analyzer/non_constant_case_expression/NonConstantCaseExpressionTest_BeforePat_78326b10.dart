void f(var e, int a) {
  switch (e) {
    case 3 + a:
//           ^
// [diag.nonConstantCaseExpression] Case expressions must be constant.
      break;
  }
}
