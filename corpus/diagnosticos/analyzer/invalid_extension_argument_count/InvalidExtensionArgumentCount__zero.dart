extension E on String {
  void m() {}
}
f() {
  E().m();
// ^^
// [diag.invalidExtensionArgumentCount] Extension overrides must have exactly one argument: the value of 'this' in the extension method.
}
