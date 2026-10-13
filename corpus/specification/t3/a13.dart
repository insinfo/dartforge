void f(num x, int? n, Object? o) {
  switch (x) {
    case int _:
      break;
    case 'a':
      break;
    case 1:
      break;
    case 1.5:
      break;
  }
  switch (n) {
    case int _:
      break;
    case 1:
      break;
    case var z:
      String s = z;
  }
  switch (o) {
    case int _:
      break;
    case String _:
      break;
    case var z:
      String s = z;
  }
}
