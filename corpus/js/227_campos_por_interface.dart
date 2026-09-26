// Um campo lido pelo tipo da interface: as classes que só a implementam
// (`implements`) têm o campo de mesmo nome noutra posição do objeto, e uma
// subclasse pode sobrescrevê-lo com getter (o caso dos tokens do
// package:yaml). A leitura e a gravação vão pela classe dinâmica.
class Token {
  final String tipo;
  final int inicio;
  String rotulo = 'base';
  Token(this.tipo, this.inicio);
}

class Escalar implements Token {
  @override
  String get tipo => 'escalar';
  @override
  final int inicio;
  final String valor;
  @override
  String rotulo = 'escalar';
  Escalar(this.inicio, this.valor);
}

class Ancora implements Token {
  final String nome;
  @override
  final String tipo = 'ancora';
  @override
  final int inicio;
  @override
  String rotulo;
  Ancora(this.nome, this.inicio) : rotulo = 'r-$nome';
}

class Derivado extends Token {
  Derivado() : super('derivado', 7);
  @override
  int get inicio => super.inicio * 10;
}

void main() {
  final tokens = <Token>[Token('t', 1), Escalar(2, 'v'), Ancora('a', 3), Derivado()];
  for (final t in tokens) {
    print('${t.tipo} ${t.inicio} ${t.rotulo}');
  }
  for (final t in tokens) {
    t.rotulo = '${t.rotulo}!';
  }
  print(tokens.map((t) => t.rotulo).join(','));
  print(tokens.map((t) => t.inicio).reduce((a, b) => a + b));
}
