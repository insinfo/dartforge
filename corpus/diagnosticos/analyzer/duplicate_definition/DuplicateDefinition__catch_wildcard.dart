f() {
  try {} catch (_, _) {}
//                 ^
// [diag.unusedCatchStack] The stack trace variable '_' isn't used and can be removed.
}