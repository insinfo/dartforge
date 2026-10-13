class A {
  void set foo(int value) {}
//         ^^^
// [context 1] The setter being overridden.
}

class B extends A {}

augment class B {
  void set foo(String value) {}
//         ^^^
// [diag.invalidOverrideSetter][context 1] The setter 'B.foo' ('void Function(String)') isn't a valid override of 'A.foo' ('void Function(int)').
}
