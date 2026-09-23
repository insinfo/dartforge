abstract class A {
  void foo(int x);
}

abstract class B {
  void foo(double x);
}

abstract class C implements A, B {
  foo(num x);
//^^^
// [diag.noCombinedSuperSignature] Can't infer missing types in 'C' from overridden methods: A.foo (void Function(int)), B.foo (void Function(double)).
}
