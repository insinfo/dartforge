enum A() {
//   ^
// [diag.finalNotInitializedConstructor1] All final variables must be initialized, but 'v2' isn't.
  e;
  final int v1;
  final int v2;
  this: v1 = 0;
}
