abstract class A {
  String foo(String a);
}

abstract class B {
  int foo(int a);
}

abstract class C implements A, B {
  foo(a);
//^^^
// [diag.noCombinedSuperSignature] Can't infer missing types in 'C' from overridden methods: A.foo (String Function(String)), B.foo (int Function(int)).
}
