typedef F = void Function<T extends num>();
void f<T extends void Function<X extends num>()>() {}
void g() {
  f<F>();
}
