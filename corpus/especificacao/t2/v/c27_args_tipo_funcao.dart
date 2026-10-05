void g<T>() {}

void f(void Function<T>() h) {
  g<int, int>();
  h<int, int>();
}
