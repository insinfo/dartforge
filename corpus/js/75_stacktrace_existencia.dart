// stack traces: só existência — catch (e, st), StackTrace.current, Error.stackTrace, Error.throwWithStackTrace.
class MeuErro extends Error {
  final String m;
  MeuErro(this.m);
}

void lanca() => throw MeuErro('x');

void main() {
  // catch (e, st) fornece um StackTrace não vazio
  try {
    throw 'texto';
  } catch (e, st) {
    print(st.toString().isNotEmpty);
    print(st is StackTrace);
  }

  // StackTrace.current fora de exceção
  final atual = StackTrace.current;
  print(atual.toString().isNotEmpty);
  print(atual is StackTrace);

  // Error.stackTrace é null antes de lançar e preenchido depois
  final erro = MeuErro('a');
  print(erro.stackTrace == null);
  try {
    throw erro;
  } catch (e, st) {
    final me = e as MeuErro;
    print(me.stackTrace != null);
    print(me.stackTrace.toString().isNotEmpty);
    print(st.toString().isNotEmpty);
  }

  // exceção lançada em função aninhada
  try {
    lanca();
  } on MeuErro catch (e, st) {
    print(e.m);
    print(st.toString().isNotEmpty);
    print(e.stackTrace != null);
  }

  // Exception (não Error) não tem stackTrace próprio, mas o catch fornece
  try {
    throw Exception('sem campo');
  } catch (e, st) {
    print(e is Exception);
    print(st.toString().isNotEmpty);
  }

  // erros do core lançados pelo runtime também trazem trace
  try {
    <int>[].first;
  } catch (e, st) {
    print(e is StateError);
    print(st.toString().isNotEmpty);
    print((e as Error).stackTrace != null);
  }
  try {
    [1][5];
  } catch (e, st) {
    print(e is RangeError);
    print(st.toString().isNotEmpty);
  }

  // Error.throwWithStackTrace: o trace fornecido é o recebido
  final st0 = StackTrace.current;
  try {
    Error.throwWithStackTrace(StateError('com trace dado'), st0);
  } catch (e, st) {
    print((e as StateError).message);
    print(identical(st, st0));
    print(st.toString() == st0.toString());
  }

  // throwWithStackTrace com objeto qualquer
  try {
    Error.throwWithStackTrace('string', st0);
  } catch (e, st) {
    print(e);
    print(identical(st, st0));
  }

  // rethrow mantém trace não vazio
  try {
    try {
      throw MeuErro('r');
    } catch (e, st) {
      print(st.toString().isNotEmpty);
      rethrow;
    }
  } catch (e, st) {
    print(st.toString().isNotEmpty);
  }

  // StackTrace.fromString
  final feito = StackTrace.fromString('trace falso');
  print(feito.toString());
  print(feito is StackTrace);
  try {
    Error.throwWithStackTrace(ArgumentError('a'), feito);
  } catch (e, st) {
    print(st.toString());
  }

  // StackTrace.empty
  print(StackTrace.empty.toString().isEmpty);

  // catch sem segundo parâmetro ainda funciona
  try {
    throw 1;
  } catch (e) {
    print('só e: $e');
  }

  // comparação de traces distintos (só que não são o mesmo objeto)
  final a = StackTrace.current;
  final b = StackTrace.current;
  print(identical(a, b));
  print('fim');
}
