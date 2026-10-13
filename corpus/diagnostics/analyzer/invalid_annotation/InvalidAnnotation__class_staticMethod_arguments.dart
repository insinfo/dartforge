class A {
  static int foo() => 0;
}

@A.foo()
// [diag.invalidAnnotation][column 1][length 8] Annotation must be either a const variable reference or const constructor invocation.
void f() {}
