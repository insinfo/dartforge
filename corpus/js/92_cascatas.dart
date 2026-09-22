// Cascatas: em construtor, métodos void, setters, []=, aninhadas, ?.., em retorno, StringBuffer/List/Map.
class Pedido {
  final List<String> itens = [];
  String cliente = '';
  int desconto = 0;
  Pedido? filho;

  void adiciona(String item) => itens.add(item);
  void limpa() => itens.clear();
  int total() => itens.length * 10 - desconto;
  Pedido comDesconto(int d) {
    desconto = d;
    return this;
  }

  @override
  String toString() => 'Pedido($cliente, $itens, desc=$desconto)';
}

class Grade {
  final List<List<int>> celulas = List.generate(2, (_) => [0, 0]);
  List<int> operator [](int i) => celulas[i];
  void operator []=(int i, List<int> v) => celulas[i] = v;
  @override
  String toString() => celulas.toString();
}

Pedido criaPedido(String nome) => Pedido()
  ..cliente = nome
  ..adiciona('a')
  ..adiciona('b');

Pedido? talvezPedido(bool cria) => cria ? Pedido() : null;

void main() {
  final p = Pedido()
    ..cliente = 'ana'
    ..adiciona('livro')
    ..adiciona('caneta')
    ..desconto = 5;
  print(p);
  print(p.total());

  p
    ..limpa()
    ..adiciona('x')
    ..comDesconto(1).adiciona('w');
  print(p);

  final q = Pedido()
    ..cliente = 'bia'
    ..filho = (Pedido()
      ..cliente = 'filho'
      ..adiciona('z'))
    ..adiciona('y');
  print(q);
  print(q.filho);

  print(criaPedido('carlos'));
  print(criaPedido('d').total());

  Pedido? nulo = talvezPedido(false);
  nulo
    ?..cliente = 'nunca'
    ..adiciona('nunca');
  print(nulo);
  Pedido? existe = talvezPedido(true);
  existe
    ?..cliente = 'sim'
    ..adiciona('ok');
  print(existe);

  final g = Grade()
    ..[0] = [1, 2]
    ..[1][1] = 9
    ..[0][0] += 10;
  print(g);

  final sb = StringBuffer()
    ..write('a')
    ..write(1)
    ..writeln('!')
    ..writeAll([1, 2, 3], ',')
    ..write(' ')
    ..writeCharCode(65);
  print(sb.toString());
  print(sb.length);

  final lista = <int>[]
    ..add(3)
    ..addAll([1, 2])
    ..sort()
    ..removeAt(0)
    ..insert(0, 0);
  print(lista);
  final lista2 = [5, 4, 3]..sort();
  print(lista2);
  print(([3, 1]..sort()).first);
  print((<int>[]..add(7)).length);

  final mapa = <String, int>{}
    ..['a'] = 1
    ..['b'] = 2
    ..putIfAbsent('c', () => 3)
    ..update('a', (v) => v + 10)
    ..remove('b');
  print(mapa);
  final conjunto = <int>{}
    ..add(1)
    ..add(1)
    ..addAll({2, 3})
    ..remove(2);
  print(conjunto);

  final ppp = Pedido();
  final mesmo = ppp..cliente = 'eu';
  print(identical(ppp, mesmo));
  final compara = (Pedido()..desconto = 3).desconto;
  print(compara);
  final contas = [1, 2, 3].map((n) => Pedido()..desconto = n).toList();
  print(contas.map((c) => c.desconto).toList());
}
