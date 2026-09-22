// Biblioteca com membros privados: _foo, classe com _campo e getter público, _Impl privada devolvida como interface pública, _mixin, construtor privado.
int _foo(int x) => x * 3;

int _contador = 0;

String usaFoo(int x) => 'foo($x)=${_foo(x)} chamadas=${++_contador}';

abstract class Servico {
  String nome();
  int executa(int v);
}

class _Impl implements Servico {
  final int _fator;
  _Impl(this._fator);
  @override
  String nome() => '_Impl(fator $_fator)';
  @override
  int executa(int v) => v * _fator;
}

class _OutraImpl extends _Impl {
  _OutraImpl() : super(-1);
  @override
  String nome() => '_OutraImpl';
}

Servico criaServico(int fator) => fator < 0 ? _OutraImpl() : _Impl(fator);

class Pessoa {
  final String _nome;
  int _idade;
  final List<String> _tags = [];
  Pessoa(this._nome, this._idade);
  Pessoa._anonima() : this('anônimo', 0);
  factory Pessoa.padrao() => Pessoa._anonima();

  String get nome => _nome;
  int get idade => _idade;
  void envelhece() => _idade++;
  void marca(String t) => _tags.add(t);
  int get quantasTags => _tags.length;

  // Métodos privados podem ser chamados por outra instância da mesma biblioteca.
  bool _maisVelhaQue(Pessoa o) => _idade > o._idade;
  bool maisVelhaQue(Pessoa o) => _maisVelhaQue(o);

  @override
  String toString() => 'Pessoa($_nome, $_idade, $_tags)';
}

mixin _Log {
  final List<String> _log = [];
  void registra(String s) => _log.add(s);
  String get historico => _log.join('|');
}

class Auditor with _Log {
  void trabalha() {
    registra('a');
    registra('b');
  }
}

enum _Modo { rapido, lento }

class Motor {
  _Modo _modo = _Modo.lento;
  void acelera() => _modo = _Modo.rapido;
  String get estado => _modo.name;
}

extension PessoaExt on Pessoa {
  String get resumo => '$_nome/$_idade';
}

// Nome privado como parâmetro nomeado não é permitido; usamos posicional.
int soma(int a, [int _b = 0]) => a + _b;

typedef _Fn = int Function(int);
_Fn _dobra = (x) => x * 2;
int aplicaDobra(int x) => _dobra(x);
