// rethrow: preserva o objeto (identical), em on e catch, aninhado, throw e vs rethrow, capturado no externo.
class Falha implements Exception {
  final String motivo;
  Falha(this.motivo);
  @override
  String toString() => 'Falha: $motivo';
}

Object? guardado;

void lancaEGuarda() {
  try {
    final f = Falha('original');
    guardado = f;
    throw f;
  } catch (e) {
    print('interno viu: $e');
    rethrow;
  }
}

void rethrowEmOn() {
  try {
    throw StateError('estado');
  } on StateError catch (e) {
    print('on StateError: ${e.message}');
    rethrow;
  }
}

void naoRethrowSeOutroTipo(Object o) {
  try {
    throw o;
  } on String catch (e) {
    print('String tratada: $e');
  } on Exception catch (e) {
    print('Exception relançada: $e');
    rethrow;
  }
}

void throwE() {
  try {
    throw Falha('throw e');
  } catch (e) {
    guardado = e;
    throw e;
  }
}

void main() {
  // identical preserva o objeto
  try {
    lancaEGuarda();
  } catch (e) {
    print('externo viu: $e');
    print(identical(e, guardado));
  }

  // rethrow em on
  try {
    rethrowEmOn();
  } catch (e) {
    print('externo: ${e is StateError}');
  }

  // só relança se casar o on de Exception
  naoRethrowSeOutroTipo('texto');
  try {
    naoRethrowSeOutroTipo(Falha('ex'));
  } catch (e) {
    print('main capturou: $e');
  }
  try {
    naoRethrowSeOutroTipo(42);
  } catch (e) {
    print('nenhum on casou, subiu direto: $e');
  }

  // throw e vs rethrow: mesmo objeto nos dois casos
  try {
    throwE();
  } catch (e) {
    print(identical(e, guardado));
    print(e);
  }

  // rethrow aninhado em vários níveis
  var niveis = 0;
  try {
    try {
      try {
        throw Falha('fundo');
      } catch (e) {
        niveis++;
        rethrow;
      }
    } catch (e) {
      niveis++;
      rethrow;
    }
  } catch (e) {
    niveis++;
    print('níveis: $niveis, $e');
  }

  // rethrow com finally: finally roda antes do catch externo
  try {
    try {
      throw 'com finally';
    } catch (e) {
      print('catch interno');
      rethrow;
    } finally {
      print('finally interno');
    }
  } catch (e) {
    print('catch externo: $e');
  }

  // rethrow condicional
  void condicional(int n) {
    try {
      throw n;
    } catch (e) {
      if (n > 5) {
        print('relançando $n');
        rethrow;
      }
      print('engolindo $n');
    }
  }

  condicional(3);
  try {
    condicional(9);
  } catch (e) {
    print('subiu $e');
  }

  // rethrow dentro de laço: sai do laço
  try {
    for (var i = 0; i < 5; i++) {
      try {
        if (i == 2) throw 'no laço $i';
        print('iteração $i');
      } catch (e) {
        rethrow;
      }
    }
  } catch (e) {
    print('laço interrompido: $e');
  }

  // rethrow preserva o stack trace (só existência)
  try {
    try {
      throw Falha('st');
    } catch (e, st) {
      print(st.toString().isNotEmpty);
      rethrow;
    }
  } catch (e, st) {
    print(st.toString().isNotEmpty);
  }

  // rethrow de objeto mutável: mutação visível fora
  final lista = <int>[];
  try {
    try {
      throw lista;
    } catch (e) {
      (e as List<int>).add(1);
      rethrow;
    }
  } catch (e) {
    print(e);
    print(identical(e, lista));
  }

  // rethrow em catch (e, st) e não-rethrow: função retorna normalmente
  String engole() {
    try {
      throw 'x';
    } catch (e, st) {
      return 'engolido ${st.toString().isNotEmpty}';
    }
  }

  print(engole());
  print('fim');
}
