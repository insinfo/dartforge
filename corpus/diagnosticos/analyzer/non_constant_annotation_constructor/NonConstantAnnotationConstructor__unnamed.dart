class A {
  A() {}
}
@A()
// [diag.nonConstantAnnotationConstructor][column 1][length 4] Annotation creation can only call a const constructor.
main() {
}
