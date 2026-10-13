mixin class A {
  A.named();
  factory A.x() = A.named;
}
class B with A {}
