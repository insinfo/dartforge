extension type BaseMethod(Object? it) {
  void foo() {}
//     ^^^
// [context 1] The method is inherited from the extension type 'BaseMethod'.
}

extension type BaseSetter(Object? it) {
  set foo(int _) {}
//    ^^^
// [context 2] The setter is inherited from the extension type 'BaseSetter'.
}

extension type Left(Object? it) implements BaseMethod {}

extension type Right(Object? it) implements BaseSetter {}

extension type C(Object? it) implements Left, Right {}
//             ^
// [diag.conflictingInheritedMethodAndSetter][context 1][context 2] The extension type 'C' can't inherit both a method and a setter named 'foo'.
