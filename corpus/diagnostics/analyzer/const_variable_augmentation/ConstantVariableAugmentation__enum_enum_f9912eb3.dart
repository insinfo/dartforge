enum E {
  foo
}
augment enum E {
  augment foo,
//        ^^^
// [diag.constantVariableAugmentation] Variable augmentations can't be const.
}
