// Padrão constante com o nome de uma constante do próprio enum (sem o
// prefixo `Enum.`), no `switch` comando e expressão, como o
// `_FfiTypeCheckDirection.reverse` do analyzer.
enum Direcao {
  ida,
  volta;

  Direcao get reverso {
    switch (this) {
      case ida:
        return volta;
      case volta:
        return ida;
    }
  }

  String get letra => switch (this) { ida => 'I', volta => 'V' };

  static String descrever(Direcao d) => switch (d) { ida => 'para lá', volta => 'para cá' };
}

enum Nivel {
  baixo(1),
  alto(10);

  final int peso;
  const Nivel(this.peso);

  bool get maximo => switch (this) { alto => true, _ => false };
}

void main() {
  print(Direcao.ida.reverso);
  print(Direcao.volta.reverso);
  print('${Direcao.ida.letra}${Direcao.volta.letra}');
  print(Direcao.descrever(Direcao.volta));
  print([for (final n in Nivel.values) '${n.name}:${n.peso}:${n.maximo}']);
}
