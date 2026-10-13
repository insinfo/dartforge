extension E on int {
  int get foo => 0;
}
extension E on int {
  int get bar => 0;
}
void f() {
  1.foo;
  1.bar;
  E(1).foo;
  E(1).bar;
}
