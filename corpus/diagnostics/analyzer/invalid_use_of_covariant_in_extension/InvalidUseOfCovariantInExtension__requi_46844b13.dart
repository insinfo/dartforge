extension E on String {
  void foo(covariant int a) {}
//         ^^^^^^^^^
// [diag.invalidUseOfCovariantInExtension] Can't have modifier 'covariant' in an extension.
}
