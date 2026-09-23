extension E on String {
  void foo([covariant int a = 0]) {}
//          ^^^^^^^^^
// [diag.invalidUseOfCovariantInExtension] Can't have modifier 'covariant' in an extension.
}
