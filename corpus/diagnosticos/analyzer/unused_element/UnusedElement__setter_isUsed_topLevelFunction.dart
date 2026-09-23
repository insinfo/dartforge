class A {
  set _value(int v) {}
}

void f() {
  A()._value = 1;
}
