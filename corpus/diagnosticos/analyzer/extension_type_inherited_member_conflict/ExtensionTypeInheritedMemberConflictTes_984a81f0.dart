extension type A1(int it) {
  void foo() {}
//     ^^^
// [context 1] Inherited from 'A1'
}

extension type A2(int it) {
  void foo() {}
//     ^^^
// [context 2] Inherited from 'A2'
}

extension type B(int it) implements A1, A2 {}
//             ^
// [diag.extensionTypeInheritedMemberConflict][context 1][context 2] The extension type 'B' has more than one distinct member named 'foo' from implemented types.
