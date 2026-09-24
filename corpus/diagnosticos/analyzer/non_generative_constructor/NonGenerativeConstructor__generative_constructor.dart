class A {
  A.named() {}
  factory A() => throw 0;
}
class B extends A {
  B() : super.named();
}
