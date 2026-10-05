void f(int x, int y) {
  const k = 1;
  final fi = 2;
  switch (x) {
    case x + 1: break;
    case y: break;
    case k: break;
    case fi: break;
    case k + y: break;
    case > y: break;
    case == fi: break;
    case < k: break;
    case >= k + y: break;
    case const (1 + y): break;
  }
  if (x case y) {}
  if (x case != y when y > 0) {}
}
