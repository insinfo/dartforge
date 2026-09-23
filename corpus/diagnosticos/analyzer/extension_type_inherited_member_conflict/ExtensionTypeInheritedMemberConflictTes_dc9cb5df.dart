class A {
  void foo(int a) {}
//     ^^^
// [context 1] Inherited from 'A'
}

class B {
  void foo(String a) {}
//     ^^^
// [context 2] Inherited from 'B'
}

class C implements A, B {
  void foo(Object a) {}
}

extension type D(C it) implements A, B {}
//             ^
// [diag.extensionTypeInheritedMemberConflict][context 1][context 2] The extension type 'D' has more than one distinct member named 'foo' from implemented types.
