class A {
  set foo(_) {}
}
class B extends A {
  foo() {}
//^^^
// [diag.conflictingMethodAndField] Class 'B' can't define method 'foo' and have field 'A.foo' with the same name.
}
