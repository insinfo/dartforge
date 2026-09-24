class A {
  final int v;
  A.named() {}
//^^^^^^^
// [diag.finalNotInitializedConstructor1] All final variables must be initialized, but 'v' isn't.
}
