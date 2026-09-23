// R-CTX-02: atribuição simples usa o tipo de escrita do alvo como contexto;
// a composta usa o tipo do parâmetro do operador; `??=` usa o tipo do alvo.
void main() {
  List<num> a = [];
  a = /*@*/[1];
  double d = 0;
  d = /*@*/1;
  /*@*/d += /*@*/1;
  num? n;
  /*@*/n ??= /*@*/1;
  List<num>? l;
  l ??= /*@*/[1];
  print([a, d, n, l]);
}
