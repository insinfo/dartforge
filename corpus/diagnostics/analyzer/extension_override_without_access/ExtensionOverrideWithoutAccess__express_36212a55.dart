class C {}
extension E on C {
  void m() {}
}
f(C c) {
  E(c);
//^^^^
// [diag.extensionOverrideWithoutAccess] An extension override can only be used to access instance members.
}
