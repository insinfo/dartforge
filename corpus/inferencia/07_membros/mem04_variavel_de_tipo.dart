// R-MEM-04: receptor variável de tipo: membros do limite.
void f<T extends List<int>, U>(T t, U u) {
  print([/*@*/t.first, /*@*/t.map((e) => e * 2.5), /*@*/u.toString(), /*@*/t[0]]);
}

void main() => f([1], 2);
