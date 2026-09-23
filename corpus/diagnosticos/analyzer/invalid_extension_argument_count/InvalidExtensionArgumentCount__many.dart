extension E on String {
  void m() {}
}
f() {
  E('a', 'b', 'c').m();
// ^^^^^^^^^^^^^^^
// [diag.invalidExtensionArgumentCount] Extension overrides must have exactly one argument: the value of 'this' in the extension method.
}
