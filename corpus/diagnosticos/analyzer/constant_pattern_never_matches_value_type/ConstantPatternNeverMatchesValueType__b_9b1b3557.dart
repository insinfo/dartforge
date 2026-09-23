void f<T>(T x) {
  if (x is bool) {
    if (x case (true)) {}
  }
}
