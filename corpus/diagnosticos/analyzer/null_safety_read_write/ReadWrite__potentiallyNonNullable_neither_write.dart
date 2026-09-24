void f<T>(bool b, T t, T t2) {
  // ignore:unused_local_variable
  T x;
  if (b) x = t;
  x = t2;
}
