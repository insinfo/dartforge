// R-LIT-02: literal de lista: E do contexto, ou UP dos elementos.
void main() {
  var a = /*@*/[];
  var b = /*@*/[1, 2.5];
  var c = /*@*/[1, null];
  var d = /*@*/<num>[/*@*/1];
  List<Object?> e = /*@*/[1];
  List<double> f = /*@*/[/*@*/1];
  var g = /*@*/[[1], [2.5]];
  var h = /*@*/const [1, 'a'];
  print([a, b, c, d, e, f, g, h]);
}
