abstract class A {
  abstract int x;
}
class B implements A {
  int get x => 0;
  void set x(int value) {}
}
