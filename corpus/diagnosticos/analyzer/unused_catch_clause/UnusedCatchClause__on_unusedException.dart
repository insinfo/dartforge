f() {
  try {
  } on String catch (exception) {
//                   ^^^^^^^^^
// [diag.unusedCatchClause] The exception variable 'exception' isn't used, so the 'catch' clause can be removed.
  }
}
