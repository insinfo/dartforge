// Dois contratos do nativo que se cruzam no handle (`Ref`):
//
// * o `int` escalar como receptor de membro do SDK chamado direto (`int` é
//   classe fechada por modificador): o `this` vai em caixa — `Smi` ou
//   `_Mint` —, nunca o `i64` cru, que seria lido como handle (o mínimo de
//   64 bits é par e negativo; um par pequeno é o handle de um slot);
// * os objetos do usuário no espaço de objetos (`heap.rs`, `Bloco`):
//   alocação em linha pela TLAB, campos lidos pelo ponteiro do objeto,
//   `hashCode` de identidade estável, objeto que sobrevive a muitas
//   coletas e lixo que volta às listas livres.
int sinal(int a) => a.sign;
int absoluto(int a) => a.abs();
bool par(int a) => a.isEven;
String texto(int a) => a.toString();
int bits(int a) => a.bitLength;

class No {
  final No? esq, dir;
  final int v;
  No(this.esq, this.dir, this.v);
  int soma() => v + (esq?.soma() ?? 0) + (dir?.soma() ?? 0);
}

No arvore(int d, int v) => d == 0 ? No(null, null, v) : No(arvore(d - 1, 2 * v), arvore(d - 1, 2 * v + 1), v);

class Vazio {
  const Vazio();
}

class Largo {
  int a = 1, b = 2, c = 3, d = 4, e = 5, f = 6, g = 7, h = 8, i = 9;
  int j = 10, k = 11, l = 12, m = 13, n = 14, o = 15, p = 16, q = 17, r = 18;
  int get total => a + b + c + d + e + f + g + h + i + j + k + l + m + n + o + p + q + r;
}

class Par<T> {
  T primeiro;
  T segundo;
  Par(this.primeiro, this.segundo);
  void trocar() {
    final t = primeiro;
    primeiro = segundo;
    segundo = t;
  }
}

void main() {
  const minimo = -9223372036854775808;
  const maximo = 9223372036854775807;
  for (final x in [minimo, minimo + 1, -4611686018427387905, -4611686018427387904, -4, -1, 0, 1, 4, 4611686018427387903, 4611686018427387904, maximo]) {
    print('$x: ${sinal(x)} ${absoluto(x)} ${par(x)} ${texto(x)} ${bits(x)}');
  }

  // Coletas com uma árvore longa viva e lixo em volta (pequeno o bastante
  // para o --gc-stress, que coleta antes de toda alocação).
  final longa = arvore(8, 1);
  var total = 0;
  for (var i = 0; i < 40; i++) {
    total += arvore(5, i).soma();
  }
  print('${longa.soma()} $total');

  // Uma lista ligada que cresce durante as coletas.
  No? cab;
  for (var i = 0; i < 3000; i++) {
    cab = No(cab, null, i);
  }
  var s = 0;
  for (var e = cab; e != null; e = e.esq) {
    s += e.v;
  }
  print(s);

  // Identidade: hash estável, objetos distintos com hash de identidade
  // próprio, objetos sem campos e com mais campos que a TLAB serve.
  final a = No(null, null, 1), b = No(null, null, 1);
  print(identical(a, b));
  print(a.hashCode == a.hashCode && identityHashCode(a) == a.hashCode);
  final conjunto = <Object>{a, b, a, const Vazio(), const Vazio(), Vazio()};
  print(conjunto.length);
  final largos = [for (var i = 0; i < 200; i++) Largo()..r = i];
  print(largos.fold<int>(0, (acc, l) => acc + l.total));

  // Campos genéricos (sempre `Ref`) com escalares e referências.
  final p = Par<Object?>(minimo, 'texto')..trocar();
  print('${p.primeiro} ${p.segundo}');
  final q = Par<int>(maximo, 4)..trocar();
  print('${q.primeiro} ${q.segundo} ${q.primeiro.sign} ${q.segundo.sign}');
}
