extension E on int {
  void m() {}
}
f() {
  E(3)..m()..m();
//^
// [diag.extensionOverrideWithCascade] Extension overrides have no value so they can't be used as the receiver of a cascade expression.
}
