extension E on int {
  int get g => 0;
}
f() {
  E(3)..g..g;
//^
// [diag.extensionOverrideWithCascade] Extension overrides have no value so they can't be used as the receiver of a cascade expression.
}
