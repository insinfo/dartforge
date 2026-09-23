abstract class A {
  external covariant num x;
}
abstract class B implements A {
  void set x(Object value); // Implicitly covariant
}
abstract class C implements B {
  int get x;
  void set x(int value); // Ok because covariant
}
