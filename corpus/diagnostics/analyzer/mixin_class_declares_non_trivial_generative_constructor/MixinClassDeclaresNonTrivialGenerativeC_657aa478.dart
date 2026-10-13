mixin class A {
  A.named();
  factory A.x() {
    return A.named();
  }
}
class B with A {}
