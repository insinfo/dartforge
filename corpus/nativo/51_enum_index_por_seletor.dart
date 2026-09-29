// `index` de um valor de enum lido pelo tipo `Enum` ou `T extends Enum` (o
// `EnumSet.updated` do analyzer): vai pela tabela de seletores da classe do
// enum, não pelo tipo estático.
enum Cor { a, b, c }

enum Modificador {
  abstrato,
  constante;
  bool get ehConst => this == constante;
}

int idx<T extends Enum>(T x) => x.index;

extension type Conj<T extends Enum>(int bits) {
  Conj<T> com(T c) => Conj<T>(bits | (1 << c.index));
}

void main() {
  print(idx(Cor.c));
  print(Conj<Modificador>(0).com(Modificador.constante).bits);
  Enum e = Cor.b;
  print(e.index);
  print(e.name);
  print(Modificador.constante.ehConst);
}
