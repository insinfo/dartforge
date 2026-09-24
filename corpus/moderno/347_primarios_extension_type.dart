// requer-dart: 3.13
// Extension type com a gramática do construtor primário (3.13): `final` na
// representação, construtor nomeado privado, `const`, corpo `;`.
extension type Id(int v) {
  int get dobro => v * 2;
}

extension type const Chave._(final String s) implements Object {
  static Chave de(String s) => Chave._(s.toLowerCase());
}

extension type Marca(String rotulo);

void main() {
  print(Id(21).dobro);
  print(Chave.de('ABC').s);
  print(Marca('m').rotulo);
  const c = Chave._('k');
  print(c.s);
}
