// R-PAD-07: padrão de objeto com classe genérica crua: o esquema é C<_> e os
// argumentos de tipo vêm do valor casado.
class Caixa<T> {
  final T valor;
  Caixa(this.valor);
}

void f(Object o) {
  var Caixa(:valor) = Caixa(1);
  print(/*@*/valor);
  if (o case Caixa(valor: var v)) print(/*@*/v);
  var Caixa<num>(valor: w) = /*@*/Caixa(2);
  print(/*@*/w);
}

void main() => f(1);
