class A {
  final int v;
  A() : this._();
  A._();
//^^^
// [diag.finalNotInitializedConstructor1] All final variables must be initialized, but 'v' isn't.
}
