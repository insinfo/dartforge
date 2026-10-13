class A implements B {
  A(int p) {}
}
class B {
  factory B(int p) = A;
}
