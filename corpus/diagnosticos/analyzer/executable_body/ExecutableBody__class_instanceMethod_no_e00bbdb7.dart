class A {
  void foo();
//^^^^^^^^^^^
// [diag.concreteClassWithAbstractMember] 'foo' must have a method body because 'A' isn't abstract.
  augment void foo();
}
