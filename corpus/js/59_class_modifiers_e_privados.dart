// Modificadores base/interface/final/sealed em uso válido; membros, classes e construtores privados.
base class Veiculo {
  final int rodas;
  Veiculo(this.rodas);
  String descreve() => 'veiculo de $rodas rodas';
}

base class Carro extends Veiculo {
  Carro() : super(4);
  @override
  String descreve() => 'carro: ${super.descreve()}';
}

final class Moto extends Veiculo {
  Moto() : super(2);
}

interface class Leitor {
  String le() => 'leitor padrao';
}

class LeitorArquivo implements Leitor {
  @override
  String le() => 'leitor de arquivo';
}

final class Token {
  final String texto;
  Token(this.texto);
  @override
  String toString() => 'Token($texto)';
}

sealed class Resultado {}

final class Sucesso extends Resultado {
  final int valor;
  Sucesso(this.valor);
}

final class Falha extends Resultado {
  final String motivo;
  Falha(this.motivo);
}

String descreveResultado(Resultado r) => switch (r) {
      Sucesso(valor: final v) => 'ok $v',
      Falha(motivo: final m) => 'falha $m',
    };

class _Escondida {
  final int _segredo;
  _Escondida(this._segredo);
  int get _dobro => _segredo * 2;
  int revela() => _dobro;
}

class Cofre {
  final int _valor;
  int _acessos = 0;
  static int _instancias = 0;

  Cofre(this._valor) {
    _instancias++;
  }

  Cofre._vazio() : _valor = 0 {
    _instancias++;
  }

  factory Cofre.vazio() => Cofre._vazio();

  int _abre() {
    _acessos++;
    return _valor;
  }

  int abrePublico() => _abre();
  int get acessos => _acessos;
  static int get instancias => _instancias;
}

int _funcaoPrivada(int x) => x + 1;

const _constantePrivada = 'priv';

void main() {
  print(Carro().descreve());
  print(Moto().descreve());
  print(Moto().rodas);
  print(Carro() is Veiculo);

  print(Leitor().le());
  print(LeitorArquivo().le());
  final Leitor l = LeitorArquivo();
  print(l.le());
  print(l is LeitorArquivo);

  print(Token('x'));
  print(Token('x').texto);

  print(descreveResultado(Sucesso(3)));
  print(descreveResultado(Falha('nao')));
  final resultados = <Resultado>[Sucesso(1), Falha('a'), Sucesso(2)];
  print(resultados.map(descreveResultado).join(' | '));
  print(resultados.whereType<Sucesso>().map((s) => s.valor).toList());

  final e = _Escondida(21);
  print(e.revela());
  print(e._segredo);
  print(e._dobro);
  print(e.runtimeType);

  final c = Cofre(7);
  print(c.abrePublico());
  print(c._abre());
  print(c._valor);
  print(c.acessos);
  print(c._acessos);
  final v = Cofre.vazio();
  print(v.abrePublico());
  print(Cofre.instancias);
  print(Cofre._instancias);
  print(Cofre._vazio()._valor);
  print(Cofre.instancias);

  print(_funcaoPrivada(1));
  print(_constantePrivada);
  final f = _funcaoPrivada;
  print(f(41));
  final lista = [1, 2].map(_funcaoPrivada).toList();
  print(lista);
}
