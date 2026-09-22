// try/catch básico: throw de String/int/objeto/Exception, catch genérico, on X catch, ordem dos on, catch (e, st).
class Meu {
  final int codigo;
  Meu(this.codigo);
  @override
  String toString() => 'Meu($codigo)';
}

int divide(int a, int b) {
  if (b == 0) throw ArgumentError('divisor zero');
  return a ~/ b;
}

void main() {
  // throw de qualquer objeto
  try {
    throw 'uma string';
  } catch (e) {
    print('capturou: $e');
    print(e is String);
  }
  try {
    throw 42;
  } catch (e) {
    print('capturou int $e');
  }
  try {
    throw Meu(7);
  } catch (e) {
    print('capturou objeto $e');
    print((e as Meu).codigo);
  }
  try {
    throw Exception('com mensagem');
  } catch (e) {
    print(e);
    print(e is Exception);
  }
  try {
    throw [1, 2];
  } catch (e) {
    print('lista: $e');
  }

  // on X catch (e)
  try {
    divide(1, 0);
  } on ArgumentError catch (e) {
    print('ArgumentError: ${e.message}');
  }
  print(divide(9, 2));

  // ordem dos on: o primeiro que casa vence
  void testaOrdem(Object o) {
    try {
      throw o;
    } on String catch (e) {
      print('String: $e');
    } on int catch (e) {
      print('int: $e');
    } on Meu catch (e) {
      print('Meu: ${e.codigo}');
    } on Exception catch (e) {
      print('Exception: $e');
    } catch (e) {
      print('outro: $e');
    }
  }

  testaOrdem('s');
  testaOrdem(3);
  testaOrdem(Meu(1));
  testaOrdem(FormatException('fmt'));
  testaOrdem(2.5);
  testaOrdem(StateError('estado'));

  // on sem catch
  try {
    throw StateError('x');
  } on StateError {
    print('on sem variável');
  }

  // on Object pega tudo, inclusive Error
  try {
    throw UnimplementedError('todo');
  } on Object catch (e) {
    print('on Object: ${e is Error}');
  }

  // catch com dois parâmetros: stack trace
  try {
    throw 'com trace';
  } catch (e, st) {
    print('e=$e');
    print(st.toString().isNotEmpty);
  }
  try {
    divide(1, 0);
  } on ArgumentError catch (e, st) {
    print('${e.message} ${st.toString().isNotEmpty}');
  }

  // throw dentro de expressão
  try {
    final x = 1 + (throw 'na expressão');
    print(x);
  } catch (e) {
    print(e);
  }
  try {
    final lista = [1, throw 'no literal', 3];
    print(lista);
  } catch (e) {
    print(e);
  }
  try {
    print('antes ${throw 'na interpolação'}');
  } catch (e) {
    print(e);
  }

  // nada lançado: catch não roda
  try {
    print('sem exceção');
  } catch (e) {
    print('não impresso');
  }

  // exceção lançada em função chamada de outra é capturada acima
  int nivel3() => throw 'do nível 3';
  int nivel2() => nivel3() + 1;
  int nivel1() => nivel2() * 2;
  try {
    nivel1();
  } catch (e) {
    print('subiu: $e');
  }

  // variável definida antes do try mantém valor após exceção
  var contador = 0;
  try {
    contador = 1;
    throw 'x';
  } catch (e) {
    contador += 10;
  }
  print(contador);

  // catch de null lançado não é possível (throw null é erro): throw de Object? cast
  try {
    Object? o = 'não nulo';
    throw o!;
  } catch (e) {
    print(e);
  }

  // e é Object no catch genérico
  try {
    throw Meu(2);
  } catch (e) {
    Object o = e;
    print(o.runtimeType == Meu);
  }
  print('fim');
}
