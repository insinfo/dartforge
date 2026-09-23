extension E on int {
  static void call() {}
}

void f() {
  E(0)();
//    ^^
// [diag.extensionOverrideAccessToStaticMember] An extension override can't be used to access a static member from an extension.
}
