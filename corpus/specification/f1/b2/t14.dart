void f() {
  var x = x;
  int y = y + 1;
  g(g);
  void g(Object o) {}
}
int a = 0;
void h() {
  print(a);
  {
    print(a);
  }
  var a = 1;
}
void k(int a) {
  void inner() { print(b); }
  var b = 0;
  inner();
}
