class A {
  final int v1;
  final int v2;
  A() {}
//^
// [diag.finalNotInitializedConstructor2] All final variables must be initialized, but 'v1' and 'v2' aren't.
}
