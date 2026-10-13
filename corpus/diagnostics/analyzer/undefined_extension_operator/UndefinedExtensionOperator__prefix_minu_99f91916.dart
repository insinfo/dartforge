extension E on String {}
f() {
  -E('a');
//^
// [diag.undefinedExtensionOperator] The operator 'unary-' isn't defined for the extension 'E'.
}
