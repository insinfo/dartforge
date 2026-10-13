abstract class A {
  abstract final num x;
}
abstract class B implements A {
  int get x;
  void set x(int value);
}
