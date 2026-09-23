// %before-language-feature: patterns
void f(int x) {
  L: switch (x) {
    case 0:
      continue L;
//    ^^^^^^^^^^^
// [diag.continueLabelInvalid] The label used in a 'continue' statement must be defined on either a loop or a switch member.
  }
}
