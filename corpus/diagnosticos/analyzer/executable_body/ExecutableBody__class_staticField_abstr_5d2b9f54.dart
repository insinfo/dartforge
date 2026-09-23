// %before-language-feature: augmentations
class A {
  static abstract int foo;
//       ^^^^^^^^
// [diag.abstractStaticField] Static fields can't be declared 'abstract'.
}
