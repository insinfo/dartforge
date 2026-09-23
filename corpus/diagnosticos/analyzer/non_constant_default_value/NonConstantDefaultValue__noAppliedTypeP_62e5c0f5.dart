void f<T>(T t) => t;

class C<T> {
  final dynamic p;
  const C({this.p = f});
}
