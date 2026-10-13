class A {
  int foo = 0;
//    ^^^
// [context 2] The setter is inherited from the class 'A'.
}

abstract class I {
  void foo();
//     ^^^
// [context 1] The method is inherited from the class 'I'.
}

extension type E(Object it) implements A, I {}
//             ^
// [diag.conflictingInheritedMethodAndSetter][context 1][context 2] The extension type 'E' can't inherit both a method and a setter named 'foo'.
//                                     ^
// [diag.extensionTypeImplementsNotSupertype] 'A' is not a supertype of 'Object', the representation type.
//                                        ^
// [diag.extensionTypeImplementsNotSupertype] 'I' is not a supertype of 'Object', the representation type.
