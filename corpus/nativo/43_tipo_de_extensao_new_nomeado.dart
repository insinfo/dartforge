// `new E._(x)` e `const E.nome(x)` de tipo de extensão: o parser não
// distingue `p.T` de `T.nome` depois de `new`/`const`; o construtor é a
// segunda parte (o `KeywordState._` do scanner do `_fe_analyzer_shared`).
extension type Estado._(int _offset) {
  static const int bloco = 59;
  bool get nulo => _offset == 0;
  Estado proximo(int n) => new Estado._(_offset + n);
  const Estado.zero() : _offset = 0;
  Estado.dobro(int x) : this._(x * 2);
}

final class Ajudante {
  static Estado get inicio => new Estado._(Estado.bloco);
  static Estado get dobro => new Estado.dobro(21);
  static const Estado zero = const Estado.zero();
}

void main() {
  final e = Ajudante.inicio;
  print([e.nulo, e.proximo(1)._offset, Ajudante.dobro._offset, Ajudante.zero.nulo]);
  print(new Estado._(3).proximo(4)._offset);
}
