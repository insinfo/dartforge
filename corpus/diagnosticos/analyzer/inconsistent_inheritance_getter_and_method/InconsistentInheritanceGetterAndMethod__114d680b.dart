abstract class A {
  int get foo;
}
abstract class B {
  int foo();
}
mixin M implements A, B {}
//    ^
// [diag.inconsistentInheritanceGetterAndMethod] 'foo' is inherited as a getter (from 'A') and also a method (from 'B').
