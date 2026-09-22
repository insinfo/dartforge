// Enum com factory (continua a ter o construtor gerador implícito) e classe
// abstrata só com factory `new` (a factory não pode ser sobreposta por um
// construtor sintético).
enum Result {
  success,
  failure;

  factory Result.parse(String name) => Result.values.byName(name);
  bool get isPassing => this == success;
  @override
  String toString() => name;
}

abstract class Client {
  factory Client() => zoneClient ?? _Impl('padrão');
  String get nome;
  void close();
}

Client? zoneClient;

class _Impl implements Client {
  @override
  final String nome;
  _Impl(this.nome);
  @override
  void close() => print('fechado $nome');
}

void main() {
  print(Result.parse('failure'));
  print(Result.success.isPassing);
  print(Result.failure.isPassing);
  print(Result.values.map((r) => r.index).toList());
  print(identical(Result.parse('success'), Result.success));
  final c = Client();
  print(c.nome);
  c.close();
  zoneClient = _Impl('zona');
  print(Client().nome);
  print(Client() is Client);
}
