// R-GEN-07: inferência dos argumentos de tipo de construtor (inclusive
// fábrica redirecionadora), como se fosse uma função genérica.
class Caixa<T> {
  T v;
  Caixa(this.v);
}

abstract class L<T> {
  factory L.de(T x) = _L<T>;
}

class _L<T> implements L<T> {
  _L(T x);
}

void main() {
  var a = /*@*/Caixa(1);
  Caixa<num> b = /*@*/Caixa(/*@*/1);
  var c = /*@*/Caixa<num>(1);
  var d = /*@*/L.de('a');
  L<Object> e = /*@*/L.de('a');
  var f = /*@*/Map.of({1: 'a'});
  print([a, b, c, d, e, f]);
}
