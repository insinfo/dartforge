// R-FLU-18: `late` e atribuição definitiva: o tipo lido é o declarado.
late int topo;
void f(bool b) {
  late int x;
  if (b) x = 1;
  late final y = [1.5];
  int z;
  if (b) {
    z = 1;
  } else {
    z = 2;
  }
  print([/*@*/x, /*@*/y, /*@*/z, /*@*/topo]);
}

void main() => f(true);
