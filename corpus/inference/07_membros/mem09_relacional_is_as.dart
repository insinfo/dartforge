// R-MEM-09: `==`, relacionais, `is`, `as`, `!` lógico.
void f(int a, Object o, bool b) {
  print([/*@*/a == o, /*@*/a < 2, /*@*/o is int, /*@*/o as num, /*@*/!b, /*@*/a != 1, /*@*/identical(a, o)]);
}

void main() => f(1, 2, true);
