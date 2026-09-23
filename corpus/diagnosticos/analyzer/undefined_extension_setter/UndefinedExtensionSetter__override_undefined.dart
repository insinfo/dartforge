extension E on int {}
f() {
  E(0).foo = 1;
//     ^^^
// [diag.undefinedExtensionSetter] The setter 'foo' isn't defined for the extension 'E'.
}
