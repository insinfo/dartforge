// toString sobrescrito em print/interpolação/join/List.toString, super.toString, default Instance of 'X'.
class SemToString {}

class Dinheiro {
  final int centavos;
  Dinheiro(this.centavos);
  @override
  String toString() =>
      'R\$ ${centavos ~/ 100},${(centavos % 100).toString().padLeft(2, '0')}';
}

class Item {
  final String nome;
  final Dinheiro preco;
  Item(this.nome, this.preco);
  @override
  String toString() => '$nome por $preco';
}

class Base {
  final int id;
  Base(this.id);
  @override
  String toString() => 'Base#$id';
}

class Derivada extends Base {
  final String extra;
  Derivada(super.id, this.extra);
  @override
  String toString() => '${super.toString()}+$extra';
}

class Neta extends Derivada {
  Neta(int id) : super(id, 'neta');
}

class SoHeranca extends Base {
  SoHeranca(super.id);
}

class Contador {
  static int chamadas = 0;
  @override
  String toString() {
    chamadas++;
    return 'c$chamadas';
  }
}

class Arvore {
  final String valor;
  final List<Arvore> filhos;
  Arvore(this.valor, [this.filhos = const []]);
  @override
  String toString() =>
      filhos.isEmpty ? valor : '$valor(${filhos.join(', ')})';
}

void main() {
  print(SemToString());
  print('${SemToString()}');
  print(SemToString().toString().startsWith("Instance of 'SemToString'"));
  print(Object().toString());

  final d = Dinheiro(1234);
  print(d);
  print('$d');
  print('preco: $d!');
  print(d.toString());
  print(d.toString().length);
  print(Dinheiro(5));
  print(Dinheiro(100));

  final itens = [Item('pao', Dinheiro(350)), Item('leite', Dinheiro(499))];
  print(itens);
  print(itens.join('; '));
  print(itens.toString());
  print('lista: $itens');
  print({'a': itens[0]});
  print({itens[1]});
  print((itens[0], 1));
  print(itens.map((i) => '$i').toList());

  print(Base(1));
  print(Derivada(2, 'x'));
  print(Neta(3));
  print(SoHeranca(4));
  final Base b = Neta(5);
  print(b);
  print('${b}${Derivada(6, 'y')}');

  final c = Contador();
  print(c);
  print('$c $c');
  print([c, c].join('-'));
  print(Contador.chamadas);
  final sb = StringBuffer();
  sb.write(c);
  sb.write(' ');
  sb.write(d);
  print(sb);
  print(sb.toString() == sb.toString());

  final arv = Arvore('raiz', [
    Arvore('a', [Arvore('a1'), Arvore('a2')]),
    Arvore('b'),
  ]);
  print(arv);
  print(arv.filhos);
  print('${arv.filhos.first}'.length);
  print(null.toString());
  print('${null}');
  print(true.toString() + false.toString());
  print(12.toString() + 'x');
  print('a'.toString());
}
