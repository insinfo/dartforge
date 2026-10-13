extension E on String {}
f() {
  E('a').m();
//       ^
// [diag.undefinedExtensionMethod] The method 'm' isn't defined for the extension 'E'.
}
