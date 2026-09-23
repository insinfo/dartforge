// R-FLU-11: switch, break/continue rotulados e junção de ramos.
void f(int? x, int k, List<int> l) {
  switch (k) {
    case 0:
      if (x == null) return;
      print(/*@*/x);
    case 1:
      print(/*@*/x);
  }
  fora:
  for (var i in l) {
    for (var j in l) {
      if (x == null) break fora;
      print([i, j, /*@*/x]);
    }
  }
  if (k == 0) {
    if (x == null) return;
  } else {
    if (x == null) return;
  }
  print(/*@*/x);
}

void main() => f(1, 0, []);
