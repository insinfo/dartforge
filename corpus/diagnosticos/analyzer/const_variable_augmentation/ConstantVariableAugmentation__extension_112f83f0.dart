extension E on int {
  static int get foo => 0;
  augment static const int foo = 0;
//                         ^^^
// [diag.constantVariableAugmentation] Variable augmentations can't be const.
}
