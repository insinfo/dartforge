class A {
  A.fromInt() {}
}
@A.fromInt()
// [diag.nonConstantAnnotationConstructor][column 1][length 12] Annotation creation can only call a const constructor.
main() {
}
