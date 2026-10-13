class A {
  void foo(num a) {}
}

class B extends A {
  void foo(dynamic a) {}
}

class C extends B {
  void foo(covariant String a) {}
//     ^^^
// [diag.invalidOverride] 'C.foo' ('void Function(String)') isn't a valid override of 'A.foo' ('void Function(num)').
}
