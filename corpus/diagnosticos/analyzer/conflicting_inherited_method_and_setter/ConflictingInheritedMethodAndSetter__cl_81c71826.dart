mixin A {
  void foo() {}
//     ^^^
// [context 1] The method is inherited from the mixin 'A'.
}

class B {
  set foo(int _) {}
//    ^^^
// [context 2] The setter is inherited from the class 'B'.
}

abstract class C with A implements B {}
//             ^
// [diag.conflictingInheritedMethodAndSetter][context 1][context 2] The class 'C' can't inherit both a method and a setter named 'foo'.
