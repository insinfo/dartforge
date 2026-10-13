class A {
  factory A() = B;
}
class B implements A {
  factory B() = C;
}
class C implements B {
  factory C() => throw 0;
}
