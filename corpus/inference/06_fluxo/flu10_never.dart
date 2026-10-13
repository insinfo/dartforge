// R-FLU-10: throw, `return` e chamada que retorna Never encerram o ramo.
Never falha() => throw 0;
void f(int? x, int? y, int? z) {
  if (x == null) throw 0;
  print(/*@*/x);
  if (y == null) falha();
  print(/*@*/y);
  if (z == null) {
    return;
  } else {
    print(/*@*/z);
  }
  print(/*@*/z);
}

void main() => f(1, 2, 3);
