typedef Fn<T> = void Function(T);

void bar() {
  (Fn<int>).foo;
}

extension E on Type {
  int get foo => 1;
}
