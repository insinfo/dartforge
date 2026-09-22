// Padrões de objeto: campos e getters, guardas compostas, relacionais, tipos com (), constantes de enum.
enum Status { ativo, inativo, bloqueado }

class Conta {
  final String dono;
  final int saldo;
  final Status status;
  final List<int> movimentos;
  Conta(this.dono, this.saldo, this.status, [this.movimentos = const []]);

  bool get negativa => saldo < 0;
  int get qtdMovimentos => movimentos.length;
  String get inicial => dono[0];
}

class Ponto {
  final int x;
  final int y;
  const Ponto(this.x, this.y);
  int get somaCoord => x + y;
}

String avalia(Conta c) => switch (c) {
      Conta(status: Status.bloqueado) => 'bloqueada',
      Conta(status: Status.inativo, saldo: 0) => 'inativa e zerada',
      Conta(status: Status.inativo) => 'inativa',
      Conta(negativa: true, :final dono) => '$dono no vermelho',
      Conta(saldo: > 1000, qtdMovimentos: > 2) => 'rica e movimentada',
      Conta(saldo: >= 1000) => 'rica',
      Conta(:final saldo, :final qtdMovimentos)
          when saldo > 100 && qtdMovimentos == 0 =>
        'parada com saldo',
      Conta(:final inicial, :final saldo) when inicial == 'a' || saldo == 7 =>
        'especial',
      Conta() => 'comum',
    };

String quadrante(Ponto p) => switch (p) {
      Ponto(x: 0, y: 0) => 'origem',
      Ponto(x: 0) || Ponto(y: 0) => 'eixo',
      Ponto(x: > 0, y: > 0) => 'Q1',
      Ponto(x: < 0, y: > 0) => 'Q2',
      Ponto(x: < 0, y: < 0) => 'Q3',
      Ponto() => 'Q4',
    };

String faixa(int n) => switch (n) {
      < 0 => 'negativo',
      0 => 'zero',
      >= 1 && <= 9 => 'digito',
      >= 10 && < 100 => 'dezena',
      != 1000 => 'grande',
      _ => 'mil',
    };

String tipo(Object o) => switch (o) {
      int() when o.isEven => 'int par',
      int() => 'int impar',
      double(isNegative: true) => 'double negativo',
      double() => 'double',
      String(isEmpty: true) => 'string vazia',
      String(length: > 3) => 'string longa',
      String() => 'string curta',
      Ponto(somaCoord: final s) when s > 10 => 'ponto longe',
      Ponto() => 'ponto',
      _ => 'outro',
    };

const origem = Ponto(0, 0);

String constante(Object o) => switch (o) {
      origem => 'a origem (const)',
      Status.ativo => 'status ativo',
      'fixo' => 'string fixa',
      42 => 'a resposta',
      _ => 'nada',
    };

void main() {
  print(avalia(Conta('ana', 5, Status.bloqueado)));
  print(avalia(Conta('bia', 0, Status.inativo)));
  print(avalia(Conta('caio', 9, Status.inativo)));
  print(avalia(Conta('dora', -10, Status.ativo)));
  print(avalia(Conta('eva', 5000, Status.ativo, [1, 2, 3])));
  print(avalia(Conta('fabio', 5000, Status.ativo, [1])));
  print(avalia(Conta('gil', 1000, Status.ativo)));
  print(avalia(Conta('hugo', 500, Status.ativo)));
  print(avalia(Conta('igor', 500, Status.ativo, [1])));
  print(avalia(Conta('ana', 50, Status.ativo, [1])));
  print(avalia(Conta('joao', 7, Status.ativo, [1])));
  print(avalia(Conta('kim', 50, Status.ativo, [1])));

  for (final p in [Ponto(0, 0), Ponto(0, 5), Ponto(3, 0), Ponto(1, 1), Ponto(-1, 1), Ponto(-1, -1), Ponto(1, -1)]) {
    print('${p.x},${p.y}: ${quadrante(p)}');
  }

  print([-5, 0, 7, 42, 500, 1000].map(faixa).join(' '));

  print(tipo(4));
  print(tipo(5));
  print(tipo(-1.5));
  print(tipo(2.5));
  print(tipo(''));
  print(tipo('abcd'));
  print(tipo('ab'));
  print(tipo(Ponto(9, 9)));
  print(tipo(Ponto(1, 1)));
  print(tipo(true));

  print(constante(Ponto(0, 0)));
  print(constante(const Ponto(0, 0)));
  print(constante(Status.ativo));
  print(constante(Status.inativo));
  print(constante('fixo'));
  print(constante(42));
  print(constante(43));

  final Object o = Conta('zoe', 3, Status.ativo, [10, 20]);
  if (o case Conta(dono: 'zoe', movimentos: [var primeiro, ...])) {
    print('zoe com primeiro movimento $primeiro');
  }
  if (o case Conta(status: Status.ativo || Status.inativo, :final dono)) {
    print('$dono nao bloqueada');
  }
  if (o case Conta(saldo: final s) when s > 100) {
    print('nunca');
  } else {
    print('saldo pequeno');
  }
}
