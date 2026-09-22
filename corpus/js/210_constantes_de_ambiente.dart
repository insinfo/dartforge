// Constantes de ambiente: sem `-D`, valem o `defaultValue` e são avaliadas na
// compilação (o `dart_sdk.js` lança se o construtor rodar em tempo de execução).
// O ramo morto de um `if` com condição constante não é emitido, como no dartdevc.
const bool depuracao = bool.fromEnvironment('modo_depuracao');
const int limite = int.fromEnvironment('limite', defaultValue: 42);
const String rotulo = String.fromEnvironment('rotulo', defaultValue: 'padrão');
const bool temChave = bool.hasEnvironment('nao_declarada');

class Config {
  static const bool verboso = bool.fromEnvironment('verboso', defaultValue: true);
  final int n;
  const Config([this.n = limite]);
  @override
  String toString() => 'Config($n, $verboso)';
}

String cliente() {
  if (const bool.fromEnvironment('sem_cliente')) {
    throw StateError('sem_cliente definido');
  }
  return 'cliente';
}

String ramoVerdadeiro() {
  if (const bool.fromEnvironment('x', defaultValue: true)) {
    return 'sim';
  } else {
    throw StateError('nunca');
  }
}

void main() {
  print(depuracao);
  print(limite);
  print(rotulo);
  print(temChave);
  print(const bool.fromEnvironment('outra'));
  print(const int.fromEnvironment('n'));
  print(const String.fromEnvironment('s'));
  print(const String.fromEnvironment('s').isEmpty);
  print(cliente());
  print(ramoVerdadeiro());
  print(Config());
  print(const Config(7));
  print(Config.verboso);
  // Em posição de expressão condicional e de valor padrão.
  print(depuracao ? 'liga' : 'desliga');
  final lista = <int>[if (const bool.fromEnvironment('inclui')) 1, 2];
  print(lista);
}
