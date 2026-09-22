// try/on/catch/finally: finally roda em todos os caminhos, exceção não casada propaga, try aninhados, ordem dos prints.
void caminho(String nome, Object? lancar) {
  print('--- $nome ---');
  try {
    try {
      print('início do try');
      if (lancar != null) throw lancar;
      print('fim do try');
    } on String catch (e) {
      print('on String: $e');
    } on FormatException catch (e) {
      print('on FormatException: ${e.message}');
    } finally {
      print('finally interno');
    }
    print('depois do try interno');
  } catch (e) {
    print('externo capturou: $e');
  } finally {
    print('finally externo');
  }
}

int comFinally(int n) {
  var log = 'a';
  try {
    if (n == 0) throw 'zero';
    log += 'b';
  } catch (e) {
    log += 'c';
  } finally {
    log += 'd';
  }
  return log.length;
}

void main() {
  caminho('sem exceção', null);
  caminho('capturada por on', 'texto');
  caminho('FormatException', FormatException('mal formatado'));
  caminho('não capturada pelos on', 42);
  caminho('Error', StateError('estado'));

  print(comFinally(1));
  print(comFinally(0));

  // finally sem catch: exceção passa depois do finally
  print('--- finally sem catch ---');
  try {
    try {
      throw 'passa';
    } finally {
      print('finally antes de propagar');
    }
  } catch (e) {
    print('capturou depois: $e');
  }

  // finally com prints e modificações visíveis
  print('--- finally modifica ---');
  var estado = 'inicial';
  try {
    estado = 'no try';
  } finally {
    estado += ' + finally';
  }
  print(estado);

  // try aninhados: interno captura, externo nem vê
  print('--- aninhado interno captura ---');
  try {
    try {
      throw 'interno';
    } catch (e) {
      print('interno capturou $e');
    }
    print('continua no externo');
  } catch (e) {
    print('externo não deve ver');
  }

  // exceção no catch interno vai para o externo, finally interno roda antes
  print('--- exceção dentro do catch ---');
  try {
    try {
      throw 'primeira';
    } catch (e) {
      print('catch interno: $e');
      throw 'segunda';
    } finally {
      print('finally interno roda antes de propagar');
    }
  } catch (e) {
    print('externo: $e');
  }

  // exceção lançada no finally substitui a anterior
  print('--- exceção no finally ---');
  try {
    try {
      throw 'original';
    } finally {
      print('finally lança');
      throw 'do finally';
    }
  } catch (e) {
    print('externo recebeu: $e');
  }

  // três níveis de finally
  print('--- três níveis ---');
  try {
    try {
      try {
        throw 'fundo';
      } finally {
        print('f3');
      }
    } finally {
      print('f2');
    }
  } catch (e) {
    print('topo: $e');
  } finally {
    print('f1');
  }

  // on com tipo genérico e catch depois
  print('--- on Exception vs catch ---');
  for (final o in [Exception('ex'), 'str', ArgumentError('arg')]) {
    try {
      throw o;
    } on Exception catch (e) {
      print('Exception: $e');
    } catch (e) {
      print('catch: ${e.runtimeType == String ? 'String' : 'Error'}');
    } finally {
      print('finally ${o.runtimeType == String}');
    }
  }

  // finally em laço conta todas as iterações
  print('--- finally em laço ---');
  var finallys = 0;
  for (var i = 0; i < 4; i++) {
    try {
      if (i.isOdd) throw i;
    } catch (e) {
      print('ímpar $e');
    } finally {
      finallys++;
    }
  }
  print('finallys=$finallys');
  print('fim');
}
