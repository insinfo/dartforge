typedef F = void Function();
typedef G<T> = List<T>;
int v = 0;
void fn() {}
void f(Object o) {
  F();
  G();
  G<int>();
  new F();
  v();
  fn.call();
  dynamic();
  Never();
}
