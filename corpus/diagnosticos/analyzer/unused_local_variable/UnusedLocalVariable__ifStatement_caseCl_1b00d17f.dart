void f(Object? x) {
  if (x case int a || [int a]) {
    a;
  }
}
