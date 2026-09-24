extension E on int {
  set foo(int _) {}
}
f() {
  E(0).foo;
//     ^^^
// [diag.undefinedExtensionGetter] The getter 'foo' isn't defined for the extension 'E'.
}
