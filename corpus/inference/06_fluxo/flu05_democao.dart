// R-FLU-05: atribuição demove (corta a cadeia de promoção) e pode promover
// para um tipo de interesse.
void f(Object o, num n, int? x) {
  if (o is int) {
    o = 'a';
    print(/*@*/o);
  }
  if (o is int) {}
  o = 1;
  print(/*@*/o);
  n = 1.5;
  print(/*@*/n);
  x = 1;
  print(/*@*/x);
  x = null;
  print(/*@*/x);
}

void main() => f('a', 1, 1);
