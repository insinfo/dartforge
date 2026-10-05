void f<T>(T x, int? m) {
  if (m case null) {
    print(3);
  } else {
    m.isEven;
  }
  if (x is int) {
    if (x case (true)) {}
    switch (x) {
      case int _:
        break;
      case 1:
        break;
    }
  }
}
