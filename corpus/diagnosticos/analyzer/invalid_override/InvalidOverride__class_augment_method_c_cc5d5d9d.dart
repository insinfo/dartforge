class A {
  void foo(num a) {}
}

class B extends A {}

augment class B {
  void foo(covariant String a) {}
//     ^^^
// [diag.invalidOverride] 'B.foo' ('void Function(String)') isn't a valid override of 'A.foo' ('void Function(num)').
}
