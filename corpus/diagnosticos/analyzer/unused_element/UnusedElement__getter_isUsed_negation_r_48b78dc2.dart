class A {
  int get _getter => 0;
}

void f() {
  var a = A();
  -(a)._getter;
}
