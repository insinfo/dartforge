// Um literal constante grande cujos elementos são outras constantes (objetos
// `const`, listas e mapas aninhados, valores de enum, `const` de topo) vira
// uma tabela de dados com os getters das constantes, que o runtime chama
// (`lower/literais.rs`, `preencher_de_tabela`; docs/NATIVO-PRODUCAO-GRANDE.md
// §6.5). A identidade canônica continua: o mesmo valor constante é o mesmo
// objeto. Fora do contexto constante, `C(1)` é um objeto novo.

enum Cor { vermelho, verde, azul }

class C {
  final int n;
  final String nome;
  const C(this.n, [this.nome = '']);
  @override
  String toString() => 'C($n${nome.isEmpty ? '' : ', $nome'})';
}

class Par<T> {
  final T a, b;
  const Par(this.a, this.b);
  @override
  String toString() => 'Par($a, $b)';
}

const um = C(1, 'um');
const grande = 1 << 40;

const mapa = {
  'a': C(1),
  'b': C(2, 'dois'),
  'c': [1, 2, 3],
  'd': {'x': 1, 'y': C(9)},
  'e': Cor.verde,
  'f': um,
  'g': Par<int>(1, 2),
  'h': -7,
  'i': 2.5,
  'j': null,
  'k': grande,
  'l': 'texto',
  'm': true,
  'n': {1, 2},
};

const lista = [
  C(1),
  C(2),
  C(3),
  Cor.azul,
  um,
  [C(1)],
  Par<String>('p', 'q'),
  'fim',
  grande,
  null,
];

const conjunto = {C(10), C(11), C(12), C(13), C(14), C(15), C(16), C(17), Cor.vermelho};

const porEnum = {
  Cor.vermelho: C(100),
  Cor.verde: C(200),
  Cor.azul: C(300),
  C(1): 'chave objeto',
  'k1': C(1),
  'k2': C(1),
  'k3': [C(1)],
  'k4': [C(1)],
};

List<Object?> naoConstante() => [C(1), C(2), C(3), C(4), C(5), C(6), C(7), C(8)];

void main() {
  print(mapa);
  print(lista);
  print(conjunto);
  print(porEnum);
  // Canônicas: a mesma constante é o mesmo objeto, dentro e fora da tabela.
  print(identical(mapa['a'], const C(1)));
  print(identical(mapa['f'], um));
  print(identical(lista[0], mapa['a']));
  print(identical((lista[5] as List)[0], const C(1)));
  print(identical(porEnum['k1'], porEnum['k2']));
  print(identical(porEnum['k3'], porEnum['k4']));
  print(identical(porEnum['k3'], const [C(1)]));
  print(identical(mapa['g'], const Par<int>(1, 2)));
  print(mapa['g'] is Par<int>);
  print(mapa['k'] == 1099511627776);
  // Imutáveis.
  try {
    (mapa['c'] as List).add(4);
  } on UnsupportedError {
    print('lista constante imutável');
  }
  try {
    (mapa as Map)['z'] = 1;
  } on UnsupportedError {
    print('mapa constante imutável');
  }
  // Fora do contexto constante, objetos novos a cada avaliação.
  final a = naoConstante(), b = naoConstante();
  print(identical(a[0], b[0]));
  print(identical(a[0], const C(1)));
  print(a);
}
