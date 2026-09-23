enum A {
  v;
  void foo() {}
}
augment enum A {
  augment foo(),
//        ^^^
// [diag.constantVariableAugmentation] Variable augmentations can't be const.
// [diag.conflictingStaticAndInstance] Class 'A' can't define static member 'foo' and have instance member 'A.foo' with the same name.
}
