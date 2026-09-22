// Factory redirecionadora usada como valor (tearoff, valor padrão de parâmetro),
// inclusive genérica e em cadeia.
class Zona {
  final String nome;
  Zona._(this.nome);
  factory Zona() = Zona._padrao;
  factory Zona._padrao() => Zona._('padrão');
  factory Zona.nomeada(String n) = Zona._;
}

class Caixa<T> {
  final T valor;
  Caixa._(this.valor);
  factory Caixa(T v) = Caixa<T>._;
  factory Caixa.dupla(T v) = _CaixaDupla<T>;
}

class _CaixaDupla<T> extends Caixa<T> {
  _CaixaDupla(T v) : super._(v);
  @override
  String toString() => 'dupla($valor)';
}

Zona cria({Zona Function() fabrica = Zona.new}) => fabrica();

void main() {
  print(cria().nome);
  print(cria(fabrica: () => Zona.nomeada('x')).nome);
  final f = Zona.nomeada;
  print(f('y').nome);
  final g = Caixa<int>.new;
  print(g(3).valor);
  final h = Caixa<String>.dupla;
  print(h('z'));
  print(Caixa.dupla(1));
  print(Caixa(2).valor);
  print([1, 2].map(Caixa<int>.new).map((c) => c.valor).toList());
}
