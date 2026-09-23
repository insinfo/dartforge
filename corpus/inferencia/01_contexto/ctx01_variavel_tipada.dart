// R-CTX-01: a variável com tipo escrito dá o contexto ao inicializador;
// `var` não dá contexto (`_`).
void main() {
  List<num> a = /*@*/[1, 2];
  Iterable<Object> b = /*@*/{1};
  var c = /*@*/[1, 2];
  final d = /*@*/<int>{};
  print([a, b, c, d]);
}
