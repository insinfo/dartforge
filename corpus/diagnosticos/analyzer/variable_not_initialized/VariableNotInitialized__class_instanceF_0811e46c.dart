class A {
  final int v1;
  final int v2;
  final int v3;
  A() {}
//^
// [diag.finalNotInitializedConstructor3Plus] All final variables must be initialized, but 'v1', 'v2', and 1 others aren't.
}
