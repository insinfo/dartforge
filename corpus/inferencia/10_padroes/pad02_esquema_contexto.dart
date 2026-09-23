// R-PAD-02: o padrão dá o esquema de contexto do inicializador.
void main() {
  var (double a, b) = (/*@*/1, /*@*/2);
  final [double x] = /*@*/[1];
  var (num c, int d) = /*@*/(1, 2);
  print([a, b, x, c, d]);
}
