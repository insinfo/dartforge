// Initializer list, super(...), super parameters, late final no construtor, asserts, ordem de avaliação.
int marca(String rotulo, int v) {
  print('avalia $rotulo=$v');
  return v;
}

class Base {
  final int base;
  final String origem;
  Base(int v)
      : base = marca('base', v),
        origem = 'Base';
  Base.nomeada(this.base) : origem = 'nomeada';
  @override
  String toString() => '$origem(base=$base)';
}

class Derivada extends Base {
  final int x;
  final int y;
  final int z = marca('z-declaracao', 100);
  late final int soma;

  Derivada(int a)
      : x = marca('x', a + 1),
        y = marca('y', 2),
        super(marca('arg-super', a * 10)) {
    soma = x + y + base;
    print('corpo Derivada');
  }

  Derivada.viaNomeada(this.x, this.y) : super.nomeada(marca('arg-nomeada', 7)) {
    soma = x + y;
  }

  @override
  String toString() => 'Derivada(x=$x, y=$y, z=$z, base=$base, soma=$soma)';
}

class Ponto {
  final int x;
  final int y;
  Ponto(this.x, this.y);
}

class Ponto3 extends Ponto {
  final int z;
  Ponto3(super.x, super.y, this.z);
  Ponto3.plano(super.x, super.y) : z = 0;
  @override
  String toString() => 'Ponto3($x, $y, $z)';
}

class Nomeado {
  final String nome;
  final int idade;
  Nomeado({required this.nome, this.idade = 0});
}

class SubNomeado extends Nomeado {
  final bool ativo;
  SubNomeado({required super.nome, super.idade, this.ativo = true});
  @override
  String toString() => 'SubNomeado($nome, $idade, $ativo)';
}

class Positivo {
  final int v;
  Positivo(this.v) : assert(v > 0, 'v deve ser positivo');
  @override
  String toString() => 'Positivo($v)';
}

class Tardio {
  late final String descricao;
  final int n;
  Tardio(this.n) {
    descricao = 'n vale $n';
  }
}

void main() {
  print(Derivada(3));
  print(Derivada.viaNomeada(1, 2));
  print(Ponto3(1, 2, 3));
  print(Ponto3.plano(4, 5));
  print(SubNomeado(nome: 'a'));
  print(SubNomeado(nome: 'b', idade: 9, ativo: false));
  print(Positivo(5));
  try {
    Positivo(-1);
    print('nao lancou');
  } catch (e) {
    print(e is AssertionError);
    print((e as AssertionError).message);
  }
  final t = Tardio(4);
  print(t.descricao);
  print(Base(1));
  print(Base.nomeada(2));
}
