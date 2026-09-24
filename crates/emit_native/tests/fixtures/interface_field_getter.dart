abstract class Nomeavel {
  String get nome;
}

abstract class Falante {
  String fala();
}

class Robo implements Nomeavel, Falante {
  final String nome;
  Robo(this.nome);
  String fala() => 'bip $nome';
}

class Papagaio implements Nomeavel, Falante {
  String get nome => 'loro';
  String fala() => 'currupaco';
}

void main() {
  final falantes = <Falante>[Robo('r2'), Papagaio()];
  for (final f in falantes) {
    print('${(f as Nomeavel).nome}: ${f.fala()}');
  }
}
