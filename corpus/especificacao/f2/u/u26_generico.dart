class A<T> {
  T? _f;
  void _m() {}
  int operator +(int o) => 0;
}
extension _E<T> on List<T> {
  void _x() {}
  List<T> operator -(int o) => this;
}
void f(A<int> a, List<int> l) {
  print(a._f);
  a._m();
  l._x();
  var m = l;
  m -= 1;
}
