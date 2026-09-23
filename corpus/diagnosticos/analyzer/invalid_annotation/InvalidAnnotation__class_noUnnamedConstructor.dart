class A {
  const A.named();
}

@A
// [diag.invalidAnnotation][column 1][length 2] Annotation must be either a const variable reference or const constructor invocation.
void f() {}
