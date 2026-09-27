// Escrita composta (`+=`, `++`, `[]` e `[]=`) em membros de tipos de
// extensão, estáticos inclusive, genéricos e cascata.
class Caixa {
  int n = 0;
}

extension type Contador(Caixa c) {
  int get valor => c.n;
  set valor(int v) => c.n = v;
  int operator [](int i) => c.n + i;
  void operator []=(int i, int v) => c.n = v - i;
  void incrementa() {
    valor += 1;
    valor++;
    ++valor;
  }

  static int total = 0;
  static void soma() {
    total += 2;
    total++;
  }
}

extension type Par<A, B>((A, B) p) {
  A get a => p.$1;
  B get b => p.$2;
  Par<B, A> troca() => Par((b, a));
  List<A> lista() => [a];
}

void main() {
  var k = Contador(Caixa());
  k.valor += 5;
  print(k.valor);
  k.valor++;
  print(k.valor);
  print(++k.valor);
  print(k.valor--);
  print(k.valor);
  k[2] += 10;
  print(k[0]);
  k.incrementa();
  print(k.valor);
  Contador.soma();
  Contador.total += 1;
  print(Contador.total);
  var p = Par((1, 'x'));
  print(p.troca().a);
  print(p.lista().runtimeType);
  print(p.troca().lista().runtimeType);
  k
    ..valor = 1
    ..incrementa();
  print(k.valor);
  Contador? talvez = k;
  talvez?.valor = 7;
  print(talvez?.valor);
}
