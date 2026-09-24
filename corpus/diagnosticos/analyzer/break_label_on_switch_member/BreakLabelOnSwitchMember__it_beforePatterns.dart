// %before-language-feature: patterns
void f(int x) {
  switch (x) {
    L: case 0:
      break;
    case 1:
      break L;
//          ^
// [diag.breakLabelOnSwitchMember] A break label resolves to the 'case' or 'default' statement.
  }
}
