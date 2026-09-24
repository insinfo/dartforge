class A {
  void foo() {}
//     ^^^
// [context 1] Inherited from 'A'
}

extension type B(A it) {
  void foo() {}
//     ^^^
// [context 2] Inherited from 'B'
}

extension type C(A it) implements A, B {}
//             ^
// [diag.extensionTypeInheritedMemberConflict][context 1][context 2] The extension type 'C' has more than one distinct member named 'foo' from implemented types.
