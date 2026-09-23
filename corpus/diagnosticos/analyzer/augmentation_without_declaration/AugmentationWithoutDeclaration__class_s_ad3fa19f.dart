class A {
  int foo = 0;
}
augment class A {
  augment static int foo = 0;
//                   ^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
// [diag.conflictingStaticAndInstance] Class 'A' can't define static member 'foo' and have instance member 'A.foo' with the same name.
}
