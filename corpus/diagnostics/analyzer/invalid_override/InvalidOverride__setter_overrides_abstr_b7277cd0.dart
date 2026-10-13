abstract class A {
  abstract covariant num x;
}
abstract class B implements A {
  int get x;
  void set x(int value);
}
