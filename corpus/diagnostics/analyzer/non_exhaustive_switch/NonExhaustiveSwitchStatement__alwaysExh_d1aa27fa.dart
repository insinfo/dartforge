void f<T>(T x) {
  if (x is bool) {
    switch (x) {
      case true:
      case false:
        break;
    }
  }
}
