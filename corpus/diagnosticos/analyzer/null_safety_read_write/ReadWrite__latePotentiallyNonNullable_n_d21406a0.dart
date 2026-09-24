void f<T>(bool b, T t) {
  late T x;
  if (b) x = t;
  x; // 0
}
