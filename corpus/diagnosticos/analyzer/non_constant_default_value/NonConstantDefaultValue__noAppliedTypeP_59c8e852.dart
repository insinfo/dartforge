void f<T>(T t) => t;

class C<T> {
  void foo([void Function<T>(T) p = f]) {}
}
