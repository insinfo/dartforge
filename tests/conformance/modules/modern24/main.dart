// Recursos posteriores ao Dart 3.6.2: curingas (3.7), elementos null-aware (3.8),
// atalhos de ponto (3.10) e construtores primários (3.13).
enum Alinhamento { esquerda, centro, direita }

class Cliente(String nome, String email) {
  String descricao() => nome;
}

class Contador(var int valor) {
  int proximo() {
    valor = valor + 1;
    return valor;
  }
}

class Legado(this.codigo) {
  final int codigo;
  int dobro() => codigo * 2;
}

class Ponto {
  final int x;
  Ponto(this.x);
  factory Ponto.origem() => Ponto(0);
}

void processar(Alinhamento a) {
  print(a.name);
}

void mostrar(Ponto p) {
  print(p.x);
}

int aplicar(int Function(int, int) f) => f(3, 4);

void main() {
  var _ = 1;
  var _ = 2;
  print(aplicar((a, _) => a + 10));

  String? ausente = null;
  String? presente = 'xyz123';
  var cabecalhos = ['Content-Type: application/json', ?ausente, ?presente];
  print(cabecalhos.length);
  print(cabecalhos[1]);

  int? vazio = null;
  var mapa = <String, int>{'a': 1, ?ausente: 2, 'c': ?vazio, 'd': 4};
  print(mapa.length);
  print(mapa['d']);

  processar(.centro);
  Alinhamento direita = .direita;
  print(direita.index);
  mostrar(.origem());
  mostrar(.new(7));

  var cliente = Cliente('Carlos', 'carlos@email.com');
  print(cliente.nome);
  print(cliente.email);
  print(cliente.descricao());
  var contador = Contador(1);
  print(contador.proximo());
  print(Legado(21).dobro());
}
