class A {
  int add(covariant int a) => a;
}
class B	extends A {
  int add(num a);
}
