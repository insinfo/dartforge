enum A() {
//   ^
// [diag.finalNotInitializedConstructor2] All final variables must be initialized, but 'v1' and 'v2' aren't.
  e;
  final int v1;
  final int v2;
}
