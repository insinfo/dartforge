class C<T> {
  final T x = y;
  const C();
}
const int y = 1;
var v = const C<String>();
