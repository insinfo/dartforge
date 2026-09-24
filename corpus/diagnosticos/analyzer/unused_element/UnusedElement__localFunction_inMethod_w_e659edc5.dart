// %before-language-feature: wildcard-variables

class C {
  m() {
    _(){}
//  ^
// [diag.unusedElement] The declaration '_' isn't referenced.
  }
}
