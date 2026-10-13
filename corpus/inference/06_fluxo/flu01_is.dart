// R-FLU-01: `is` promove no ramo verdadeiro; `is!` com saída promove depois.
void f(Object o) {
  if (o is int) {
    print(/*@*/o);
  }
  print(/*@*/o);
  if (o is! String) return;
  print(/*@*/o);
}

void g(num n) {
  if (n is String) {
    print(/*@*/n);
  }
}

void main() => f('a');
