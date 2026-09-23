class A {
  static f() {}
}
@A.f
// [diag.invalidAnnotation][column 1][length 4] Annotation must be either a const variable reference or const constructor invocation.
main() {
}
