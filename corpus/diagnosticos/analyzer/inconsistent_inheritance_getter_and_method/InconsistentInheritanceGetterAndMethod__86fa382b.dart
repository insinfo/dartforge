abstract class A {
  int foo();
}
abstract class B {
  int get foo;
}
mixin M implements A, B {}
//    ^
// [diag.inconsistentInheritanceGetterAndMethod] 'foo' is inherited as a getter (from 'B') and also a method (from 'A').
