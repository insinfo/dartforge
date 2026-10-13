extension E on String {
  void m() {}
}
f() {
  E(null).m();
//  ^^^^
// [diag.extensionOverrideArgumentNotAssignable] The type of the argument to the extension override 'Null' isn't assignable to the extended type 'String'.
}
