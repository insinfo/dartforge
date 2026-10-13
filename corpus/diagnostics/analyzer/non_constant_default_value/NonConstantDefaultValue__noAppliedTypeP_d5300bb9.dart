void f<T>(T t) => t;

class C<T> {
  final void Function<T>(T) p;
  const C({this.p = f});
}
