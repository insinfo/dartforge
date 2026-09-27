// Membros de tipos de extensão (Dart 3.3) apagados: getters, setters,
// métodos, operadores, construtores (primário, redirecionador, com corpo,
// factory), estáticos, tearoffs, genéricos e `implements` do tipo de
// representação.
extension type Id(int v) {
  int get dobro => v * 2;
  Id operator +(Id o) => Id(v + o.v);
  int operator [](int i) => v * i;
  void operator []=(int i, int x) => print('set[$i]=$x em $v');
  Id operator -() => Id(-v);
  bool get par => v.isEven;
  int soma(int a, [int b = 10, int c = 100]) => v + a + b + c;
  String nomeado({required String prefixo, String sufixo = '!'}) => '$prefixo$v$sufixo';
  int get isEven => 42;
  set rotulo(String s) => print('rotulo $s em $v');
  Id.dobrado(int x) : this(x * 2);
  Id.comCorpo(int x) : v = x + 1 {
    print('corpo $v');
    if (v > 100) print('grande');
    print('fim');
  }
  factory Id.fabrica(int x) => Id(x * 10);
  factory Id.redir(int x) = Id.dobrado;
  static int contador = 0;
  static Id zero() => Id(0);
  static const um = 1;
  int usaEstatico() => v + um + contador;
  int chamaMembro() => dobro + soma(1) + this.dobro;
}

extension type Caixa<T>(List<T> itens) {
  T get primeiro => itens.first;
  Caixa<T> mais(T x) => Caixa([...itens, x]);
  R mapear<R>(R Function(T) f) => f(primeiro);
}

extension type Nome(String s) implements String {
  String get grito => '${toUpperCase()}!';
}

extension type const Chave._(String s) implements Object {
  static Chave de(String s) => Chave._(s.toLowerCase());
}

void main() {
  var a = Id(3);
  print(a.dobro);
  print((a + Id(4)).v);
  print(a[5]);
  a[1] = 9;
  print((-a).v);
  print(a.par);
  print(a.soma(1));
  print(a.soma(1, 2));
  print(a.soma(1, 2, 3));
  print(a.nomeado(prefixo: '<'));
  print(a.isEven);
  a.rotulo = 'x';
  var r = (a.rotulo = 'y');
  print(r);
  print(Id.dobrado(5).v);
  print(Id.comCorpo(5).v);
  print(Id.comCorpo(500).v);
  print(Id.fabrica(2).v);
  print(Id.redir(7).v);
  Id.contador = 5;
  print(Id.zero().v);
  print(a.usaEstatico());
  print(a.chamaMembro());
  var f = a.soma;
  print(f(0));
  var g = Id.new;
  print(g(8).dobro);
  var h = Id.dobrado;
  print(h(8).v);
  var c = Caixa([1, 2]);
  print(c.primeiro);
  print(c.mais(3).itens);
  print(c.mapear((x) => 'v$x'));
  print(c.itens.runtimeType);
  var n = Nome('ana');
  print(n.grito);
  print(n.length);
  print(Chave.de('ABC').s);
  const k = Chave._('k');
  print(k.s);
  Id? talvez = null;
  print(talvez?.dobro);
  talvez = Id(1);
  print(talvez?.dobro);
  print(a is int);
  print(a == Id(3));
  print([Id(1), Id(2)].map((i) => i.dobro).toList());
}
