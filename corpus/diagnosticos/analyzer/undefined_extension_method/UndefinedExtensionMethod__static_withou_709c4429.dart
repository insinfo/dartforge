extension E on Object {}
void f() {
  E.m();
//  ^
// [diag.undefinedExtensionMethod] The method 'm' isn't defined for the extension 'E'.
}
