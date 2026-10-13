enum A {
  e;
  final int v;
  const A();
//      ^
// [diag.finalNotInitializedConstructor1] All final variables must be initialized, but 'v' isn't.
}
