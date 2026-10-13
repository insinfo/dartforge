class A {
  foo() {}
}
class B extends A {
  int foo = 0;
//    ^^^
// [diag.conflictingFieldAndMethod] Class 'B' can't define field 'foo' and have method 'A.foo' with the same name.
}
