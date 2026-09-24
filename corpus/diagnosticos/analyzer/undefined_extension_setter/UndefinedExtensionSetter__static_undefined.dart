extension E on int {}
void f() {
  E.foo = 3;
//  ^^^
// [diag.undefinedExtensionSetter] The setter 'foo' isn't defined for the extension 'E'.
}
