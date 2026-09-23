void f(Object? x) {
  switch (x) {
    case (int a,) when a > 0:
    case [int _]:
      break;
  };
}
