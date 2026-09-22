// toString de erros e exceções construídos com mensagem própria: Exception, FormatException, StateError, ArgumentError, etc.
class ErroProprio extends Error {
  final String detalhe;
  ErroProprio(this.detalhe);
  @override
  String toString() => 'ErroProprio: $detalhe';
}

class ErroSemToString extends Error {}

class ExcecaoProprio implements Exception {
  final int n;
  ExcecaoProprio(this.n);
  @override
  String toString() => 'ExcecaoProprio #$n';
}

void main() {
  print(ErroProprio('detalhe'));
  print(ErroProprio('x').toString().length);
  print(ErroSemToString().toString() == "Instance of 'ErroSemToString'");
  print(ExcecaoProprio(3));

  print(Exception('msg'));
  print(Exception());
  print(Exception(42));
  print(Exception('multi\npalavra'));
  print(Exception(['a', 1]));

  print(FormatException('m'));
  print(FormatException());
  print(FormatException('com fonte', 'abcdef'));
  print(FormatException('com offset', 'abcdef', 3));
  print(FormatException('', 'só fonte'));

  print(StateError('m'));
  print(StateError(''));

  print(ArgumentError('m'));
  print(ArgumentError());
  print(ArgumentError('m', 'nome'));
  print(ArgumentError.value(5, 'n', 'msg'));
  print(ArgumentError.value(5, 'n'));
  print(ArgumentError.value(5));
  print(ArgumentError.value('texto', 'nome', 'mensagem'));
  print(ArgumentError.value(null, 'n', 'nulo'));
  print(ArgumentError.notNull('p'));
  print(ArgumentError.notNull());

  print(UnsupportedError('m'));
  print(UnimplementedError('m'));
  print(UnimplementedError());

  print(RangeError('m'));
  print(RangeError.value(7));
  print(RangeError.value(7, 'idx'));
  print(RangeError.value(7, 'idx', 'fora'));
  print(RangeError.range(15, 0, 10));
  print(RangeError.range(15, 0, 10, 'v'));
  print(RangeError.range(15, 0, 10, 'v', 'msg'));
  print(RangeError.index(5, [1, 2, 3]));
  print(RangeError.index(5, [1, 2, 3], 'index'));
  print(RangeError.index(5, [1, 2, 3], 'index', 'msg'));
  print(IndexError.withLength(5, 3));
  print(IndexError.withLength(5, 3, name: 'i'));

  print(AssertionError('m'));
  print(AssertionError());
  print(AssertionError(42));

  print(ConcurrentModificationError());

  print(TypeError() is Error);

  // mensagens com interpolação e objetos
  final obj = ExcecaoProprio(9);
  print(StateError('objeto: $obj'));
  print(Exception(obj));
  print(ArgumentError.value(obj, 'o'));

  // message/name acessíveis
  final a = ArgumentError.value(5, 'n', 'msg');
  print('${a.name} ${a.invalidValue} ${a.message}');
  final r = RangeError.range(15, 0, 10, 'v', 'msg');
  print('${r.name} ${r.invalidValue} ${r.start} ${r.end} ${r.message}');
  final f = FormatException('m', 'src', 1);
  print('${f.message} ${f.source} ${f.offset}');
  print(StateError('sm').message);
  print(UnsupportedError('um').message);
  print(UnimplementedError('im').message);
  print(UnimplementedError().message);
  print(AssertionError('am').message);
  print(AssertionError().message);

  // toString via catch
  for (final e in <Object>[
    Exception('a'),
    StateError('b'),
    ArgumentError('c'),
    FormatException('d'),
    UnsupportedError('e'),
    ErroProprio('f'),
  ]) {
    try {
      throw e;
    } catch (x) {
      print('capturado: $x');
    }
  }
  print('fim');
}
