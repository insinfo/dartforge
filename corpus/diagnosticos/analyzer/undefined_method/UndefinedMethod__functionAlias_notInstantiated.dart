typedef Fn<T> = void Function(T);

void bar() {
  Fn.foo();
}

extension E on Type {
  void foo() {}
}
