extension type A(Object? it) {
  void foo() {}
//     ^^^
// [context 1] The method is inherited from the extension type 'A'.
}

extension type B(Object? it) {
  set foo(int _) {}
//    ^^^
// [context 2] The setter is inherited from the extension type 'B'.
}

extension type C(Object? it) implements A, B {
//             ^
// [diag.conflictingInheritedMethodAndSetter][context 1][context 2] The extension type 'C' can't inherit both a method and a setter named 'foo'.
  void bar() {}
}
