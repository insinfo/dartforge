void f() {
  L:
  {
    for (var i in []) {
//           ^
// [diag.unusedLocalVariable] The value of the local variable 'i' isn't used.
      continue L;
//    ^^^^^^^^^^^
// [diag.continueLabelInvalid] The label used in a 'continue' statement must be defined on either a loop or a switch member.
    }
  }
}
