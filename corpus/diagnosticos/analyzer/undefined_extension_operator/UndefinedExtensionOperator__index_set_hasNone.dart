class A {}

extension E on A {}

f(A a) {
  E(a)[0] = 1;
//    ^^^
// [diag.undefinedExtensionOperator] The operator '[]=' isn't defined for the extension 'E'.
}
