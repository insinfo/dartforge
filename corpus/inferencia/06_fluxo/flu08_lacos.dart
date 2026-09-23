// R-FLU-08: laços: variável atribuída no corpo é demovida na entrada do laço.
void f(int? x, int? y, List<int> l) {
  if (x == null || y == null) return;
  while (l.isEmpty) {
    print(/*@*/x);
    print(/*@*/y);
    x = null;
  }
  for (var i in l) {
    print(/*@*/y);
    print(i);
  }
  do {
    print(/*@*/y);
    y = null;
  } while (l.isEmpty);
}

void main() => f(1, 2, []);
