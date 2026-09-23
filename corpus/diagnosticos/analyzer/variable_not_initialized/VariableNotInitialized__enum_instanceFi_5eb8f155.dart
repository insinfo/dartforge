enum A {
  v1, v2._();
  final int v;
  const A() : this._();
  const A._();
//      ^^^
// [diag.finalNotInitializedConstructor1] All final variables must be initialized, but 'v' isn't.
}
