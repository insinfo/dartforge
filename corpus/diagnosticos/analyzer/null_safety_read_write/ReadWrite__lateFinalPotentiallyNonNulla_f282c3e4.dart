void f<T>(bool b, T t) {
  late final T x;
  if (b) x = t;
  x; // 0
}
