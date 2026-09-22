// assert: passa, com mensagem, falha capturada em try/catch, assert em initializer list de construtor.
class Positivo {
  final int valor;
  Positivo(this.valor) : assert(valor > 0, 'valor deve ser positivo');
  Positivo.semMensagem(this.valor) : assert(valor > 0);
}

int lento() {
  print('avaliou condição');
  return 1;
}

void main() {
  assert(true);
  assert(1 + 1 == 2, 'aritmética');
  print('asserts passaram');

  // condição de assert é avaliada quando asserts estão ativos
  assert(lento() == 1);

  // assert que falha capturado
  try {
    assert(1 > 2, 'um não é maior que dois');
    print('não deveria chegar aqui');
  } catch (e) {
    print(e is AssertionError);
    print((e as AssertionError).message);
  }

  // assert com mensagem dinâmica
  final x = 5;
  try {
    assert(x < 3, 'x=$x não é menor que 3');
  } on AssertionError catch (e) {
    print(e.message);
  }

  // assert sem mensagem: message é null
  try {
    assert(false);
  } on AssertionError catch (e) {
    print('sem mensagem: ${e.message == null}');
  }

  // assert é Error, não Exception
  try {
    assert(false, 'tipo');
  } catch (e) {
    print(e is Error);
    print(e is Exception);
  }

  // assert em construtor
  final p = Positivo(3);
  print(p.valor);
  try {
    Positivo(-1);
    print('não construiu');
  } on AssertionError catch (e) {
    print('construtor: ${e.message}');
  }
  try {
    Positivo.semMensagem(0);
  } catch (e) {
    print('semMensagem lançou ${e is AssertionError}');
  }

  // assert dentro de laço: só o terceiro falha
  var passou = 0;
  for (var i = 0; i < 5; i++) {
    try {
      assert(i != 3, 'i era três');
      passou++;
    } on AssertionError catch (e) {
      print('falhou em $i: ${e.message}');
    }
  }
  print('passou $passou');

  // assert com mensagem que é objeto não-string
  try {
    assert(false, 42);
  } on AssertionError catch (e) {
    print('message=${e.message} tipo int? ${e.message is int}');
  }

  // assert em função local
  void checa(int n) {
    assert(n.isEven, 'esperava par, veio $n');
  }

  checa(2);
  try {
    checa(3);
  } on AssertionError catch (e) {
    print(e.message);
  }
  print('fim');
}
