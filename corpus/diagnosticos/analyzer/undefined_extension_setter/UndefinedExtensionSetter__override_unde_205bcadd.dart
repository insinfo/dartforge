class C {
  set foo(int value) {}
}
extension E on C {}
void f(C c) {
  E(c).foo = 1;
//     ^^^
// [diag.undefinedExtensionSetter] The setter 'foo' isn't defined for the extension 'E'.
}
