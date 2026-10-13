abstract class A {
  int foo(int x);
}

abstract class B {
  double foo(int x);
}

abstract class C implements A, B {
  Never foo(x);
//      ^^^
// [diag.noCombinedSuperSignature] Can't infer missing types in 'C' from overridden methods: A.foo (int Function(int)), B.foo (double Function(int)).
}
