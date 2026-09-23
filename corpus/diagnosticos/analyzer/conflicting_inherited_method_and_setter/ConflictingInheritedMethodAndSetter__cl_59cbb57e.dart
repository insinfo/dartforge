class A {
  void foo() {}
//     ^^^
// [context 1] The method is inherited from the class 'A'.
}

mixin B {
  set foo(int _) {}
//    ^^^
// [context 2] The setter is inherited from the mixin 'B'.
}

abstract class C extends A with B {}
//             ^
// [diag.conflictingInheritedMethodAndSetter][context 1][context 2] The class 'C' can't inherit both a method and a setter named 'foo'.
