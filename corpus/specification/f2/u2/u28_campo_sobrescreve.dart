class A {
  int get _x => 0;
  int _y = 0;
}
class B extends A {
  @override
  int _x = 1;
  @override
  int _y = 2;
  int _z = 3;
}
void f(A a, dynamic d) {
  print(a._x);
  print(d._z);
}
