abstract class A {
  int get foo;
}
abstract class B {
  int foo();
}
abstract class C implements A, B {
  int foo() => 0;
//    ^^^
// [diag.inconsistentInheritanceGetterAndMethod] 'foo' is inherited as a getter (from 'A') and also a method (from 'B').
}
