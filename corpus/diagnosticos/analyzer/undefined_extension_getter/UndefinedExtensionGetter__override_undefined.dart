extension E on String {}
f() {
  E('a').g;
//       ^
// [diag.undefinedExtensionGetter] The getter 'g' isn't defined for the extension 'E'.
}
