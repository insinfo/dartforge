// Classes abstratas, implements de classe concreta, múltiplas interfaces, template method, is.
abstract class Forma {
  final String nome;
  Forma(this.nome);

  double area();
  String get tipo;

  String descreve() => '$nome ($tipo) area=${area().toStringAsFixed(2)}';

  bool maiorQue(Forma outra) => area() > outra.area();
}

class Retangulo extends Forma {
  final double w;
  final double h;
  Retangulo(this.w, this.h) : super('retangulo');
  @override
  double area() => w * h;
  @override
  String get tipo => 'poligono';
}

class Circulo extends Forma {
  final double r;
  Circulo(this.r) : super('circulo');
  @override
  double area() => 3.14159 * r * r;
  @override
  String get tipo => 'curva';
}

abstract class Relatorio {
  void gera() {
    print('cabecalho: ${titulo()}');
    for (final linha in linhas()) {
      print('  $linha');
    }
    print('rodape: ${linhas().length} linhas');
  }

  String titulo();
  List<String> linhas();
}

class RelatorioVendas extends Relatorio {
  @override
  String titulo() => 'Vendas';
  @override
  List<String> linhas() => ['a=1', 'b=2'];
}

abstract class Nomeavel {
  String get nome;
}

abstract class Falante {
  String fala();
}

abstract class Movel {
  int velocidade();
}

class Robo implements Nomeavel, Falante, Movel {
  @override
  final String nome;
  Robo(this.nome);
  @override
  String fala() => 'bip $nome';
  @override
  int velocidade() => 3;
}

class Pessoa {
  final String nome;
  Pessoa(this.nome);
  String cumprimenta() => 'oi, sou $nome';
}

class Impostor implements Pessoa {
  @override
  String get nome => 'anonimo';
  @override
  String cumprimenta() => 'nao digo quem sou';
}

class Papagaio implements Falante, Nomeavel {
  @override
  String get nome => 'loro';
  @override
  String fala() => 'currupaco';
}

void main() {
  final formas = <Forma>[Retangulo(2, 3.5), Circulo(1.5), Retangulo(1, 1.25)];
  for (final f in formas) {
    print(f.descreve());
  }
  print(formas[0].maiorQue(formas[1]));
  print(formas[1].maiorQue(formas[2]));
  print(formas.map((f) => f.tipo).toSet());

  RelatorioVendas().gera();
  final Relatorio r = RelatorioVendas();
  print(r.titulo());
  print(r is Relatorio);

  final robo = Robo('r2');
  print(robo.fala());
  print(robo.velocidade());
  print(robo.nome);
  print(robo is Nomeavel);
  print(robo is Falante);
  print(robo is Movel);
  final Falante fal = robo;
  print(fal.fala());

  final Pessoa p = Impostor();
  print(p.nome);
  print(p.cumprimenta());
  print(p is Impostor);
  print(Pessoa('x') is Impostor);

  final falantes = <Falante>[robo, Papagaio()];
  for (final f in falantes) {
    print('${(f as Nomeavel).nome}: ${f.fala()}');
  }
  final nomes = falantes.whereType<Nomeavel>().map((n) => n.nome).toList();
  print(nomes);
  final Object o = Papagaio();
  print(o is Falante && o is Nomeavel);
  print(o is Movel);
}
