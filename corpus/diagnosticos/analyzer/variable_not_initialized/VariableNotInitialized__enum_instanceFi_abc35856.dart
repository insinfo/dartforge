enum A() {
//   ^
// [diag.finalNotInitializedConstructor1] All final variables must be initialized, but 'v' isn't.
  e;
  final int v;
  this;
}
