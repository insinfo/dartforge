extension E on String {}
f() {
  E('a') + 1;
//       ^
// [diag.undefinedExtensionOperator] The operator '+' isn't defined for the extension 'E'.
}
