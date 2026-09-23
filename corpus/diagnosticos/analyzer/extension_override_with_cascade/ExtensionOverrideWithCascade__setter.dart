extension E on int {
  set s(int i) {}
}
f() {
  E(3)..s = 1..s = 2;
//^
// [diag.extensionOverrideWithCascade] Extension overrides have no value so they can't be used as the receiver of a cascade expression.
}
