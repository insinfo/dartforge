void f() {
  try {} catch (exception, stackTrace) {
//                         ^^^^^^^^^^
// [diag.unusedCatchStack] The stack trace variable 'stackTrace' isn't used and can be removed.
  }
}
