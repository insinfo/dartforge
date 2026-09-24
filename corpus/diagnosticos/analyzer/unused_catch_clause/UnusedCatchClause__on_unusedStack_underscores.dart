f() {
  try {
  } on String catch (exception, __) {
//                              ^^
// [diag.unusedCatchStack] The stack trace variable '__' isn't used and can be removed.
  }
}
