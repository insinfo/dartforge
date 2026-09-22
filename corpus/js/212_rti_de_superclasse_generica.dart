// Classe sem parâmetros de tipo próprios cuja superclasse é genérica: o
// construtor e as factories recebem o `rti` mesmo assim (o DDC passa `_ti`
// quando a classe *ou uma superclasse* é genérica).
import 'dart:async';

class Caixa<T> {
  final T valor;
  Caixa(this.valor);
  @override
  String toString() => 'Caixa($valor)';
}

class CaixaDeInt extends Caixa<int> {
  CaixaDeInt(super.valor);
  factory CaixaDeInt.zero() => CaixaDeInt(0);
  factory CaixaDeInt.doDobro(int v) = _CaixaDobro;
  int get dobro => valor * 2;
}

class _CaixaDobro extends CaixaDeInt {
  _CaixaDobro(int v) : super(v * 2);
}

class Fluxo extends StreamView<List<int>> {
  Fluxo(super.stream);
  factory Fluxo.deBytes(List<int> bytes) => Fluxo(Stream.value(bytes));
}

Future<void> main() async {
  final c = CaixaDeInt(21);
  print(c);
  print(c.dobro);
  print(CaixaDeInt.zero());
  print(CaixaDeInt.doDobro(5));
  print(c is Caixa<int>);
  print(c.runtimeType);

  // Tearoffs de construtor e de factory.
  final nova = CaixaDeInt.new;
  print(nova(3));
  final zero = CaixaDeInt.zero;
  print(zero());
  print([1, 2, 3].map(CaixaDeInt.new).map((x) => x.dobro).toList());

  final f = Fluxo.deBytes([1, 2, 3]);
  print(await f.first);
  print(f is Stream<List<int>>);
  final g = Fluxo(Stream.value(<int>[4, 5]));
  print(await g.first);
}
