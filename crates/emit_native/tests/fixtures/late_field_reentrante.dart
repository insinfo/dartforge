class Reentrante {
  late String direta = direta;
  late String indireta = lerIndireta();
  String lerIndireta() => indireta;

  int inicializacoes = 0;
  late String? nula = inicializarNula();
  String? inicializarNula() {
    inicializacoes++;
    return null;
  }

  late String durante = inicializarDurante();
  String inicializarDurante() {
    durante = 'temporario';
    print(durante);
    return 'final';
  }

  int tentativas = 0;
  late String recuperavel = inicializarRecuperavel();
  String inicializarRecuperavel() {
    tentativas++;
    if (tentativas == 1) throw StateError('primeira');
    return 'pronto';
  }
}

void main() {
  final c = Reentrante();
  for (var i = 0; i < 2; i++) {
    try { print(c.direta); } catch (e) { print(e is StackOverflowError); print(e); }
    try { print(c.indireta); } catch (e) { print(e is StackOverflowError); print(e); }
  }
  print(c.nula == null);
  print(c.nula == null);
  print(c.inicializacoes);
  print(c.durante);
  for (var i = 0; i < 2; i++) {
    try { print(c.recuperavel); } catch (e) { print(e); }
  }
  print(c.tentativas);
  final outro = Reentrante();
  outro.nula = null;
  print(outro.nula == null);
  print(outro.inicializacoes);
}
