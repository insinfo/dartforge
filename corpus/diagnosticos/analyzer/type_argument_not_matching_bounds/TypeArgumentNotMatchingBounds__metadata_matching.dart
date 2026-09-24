class A<T extends num> {
  const A();
}

@A<int>()
void f() {}
