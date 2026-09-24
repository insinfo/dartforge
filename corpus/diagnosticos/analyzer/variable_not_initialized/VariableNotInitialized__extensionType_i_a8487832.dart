extension type A(int it) {
  A.named();
//^^^^^^^
// [diag.finalNotInitializedConstructor1] All final variables must be initialized, but 'it' isn't.
}
