void f(Object? x) {
  switch (x) {
    case <int>[var a] when a > 0:
    case <int>[var a, 0] when a > 0:
      a;
  }
}
