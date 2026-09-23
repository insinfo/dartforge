enum A(this.v1) {
//   ^
// [diag.finalNotInitializedConstructor1] All final variables must be initialized, but 'v2' isn't.
  e(0);
  final int v1;
  final int v2;
}
