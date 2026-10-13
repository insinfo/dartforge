// R-MEM-03: receptor dynamic: resultado dynamic, salvo membros de Object;
// receptor Never: resultado Never.
Never falha() => throw 0;
void f(dynamic d, bool b) {
  print([/*@*/d.foo(), /*@*/d.toString(), /*@*/d.hashCode, /*@*/d.runtimeType, /*@*/d.bar, /*@*/d[0], /*@*/d + 1, /*@*/d == 1]);
  if (b) print(/*@*/falha().x);
}

void main() => f(1, false);
