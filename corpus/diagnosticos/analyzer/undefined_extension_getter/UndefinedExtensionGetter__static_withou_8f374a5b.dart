extension E on Object {}
void f() {
  E.v;
//  ^
// [diag.undefinedExtensionGetter] The getter 'v' isn't defined for the extension 'E'.
}
