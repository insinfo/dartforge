// %before-language-feature: wildcard-variables

void f() {
  _(){}
//^
// [diag.unusedElement] The declaration '_' isn't referenced.
}
