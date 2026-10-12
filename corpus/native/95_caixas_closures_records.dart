// Caixas, closures e records no espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md
// §2.10, §5.3): `identical` de `double` (1.0, -0.0, NaN) pelos bits, inteiros
// nos limites do Smi (±2^62, 2^63-1, -2^63) com aritmética e `identical` por
// valor, `double` e `int` grande em `List<Object>`, closures que capturam
// `int`/`double`/variáveis mutáveis, tear-offs iguais e diferentes (`==` e
// `identical`), `Function.apply` e records posicionais e nomeados (`==`, igualdade
// do `hashCode` entre iguais, `toString`).

int soma(int a, int b) => a + b;
int soma2(int a, int b) => a + b;

String nomeado(int a, {int b = 10, String c = 'c'}) => 'a=$a b=$b c=$c';

String opcional(int a, [int? b, double c = 1.5]) => 'a=$a b=$b c=$c';

class Contador {
  int n;
  Contador(this.n);
  int incrementa(int d) => n += d;
  static int dobro(int x) => x * 2;
}

double id(double x) => x;
int idInt(int x) => x;
Object ida(Object x) => x;

void tentar(String nome, Object? Function() f) {
  try {
    print('$nome: ${f()}');
  } catch (e) {
    print('$nome: ${e.runtimeType}');
  }
}

void main() {
  // double: identidade pelos bits.
  final um = id(1.0);
  final umB = double.parse('1.0');
  print('identical(1.0, 1.0): ${identical(1.0, 1.0)} ${identical(um, umB)} ${identical(um, 1.0)}');
  final zero = id(0.0);
  final menosZero = id(-0.0);
  print('0.0 == -0.0: ${zero == menosZero} identical: ${identical(zero, menosZero)} '
      'identical(-0.0,-0.0): ${identical(menosZero, -0.0)}');
  print('-0.0: $menosZero isNegative: ${menosZero.isNegative} 1/-0.0: ${1 / menosZero}');
  print('compareTo: ${zero.compareTo(menosZero)} ${menosZero.compareTo(zero)}');
  final nan = id(double.nan);
  print('NaN == NaN: ${nan == nan} identical: ${identical(nan, nan)} ${identical(nan, double.nan)}');
  print('NaN em lista: ${[nan].contains(nan)} indexOf: ${[1.0, nan].indexOf(nan)}');
  print('NaN compareTo: ${nan.compareTo(nan)} ${nan.compareTo(double.infinity)}');
  print('int == double: ${idInt(1) == um} identical: ${identical(1, um)}');
  print('hashCode int/double: ${1.hashCode == 1.0.hashCode} ${um.hashCode == 1.0.hashCode}');
  print('hashCode constantes: ${null.hashCode} ${true.hashCode} ${false.hashCode}');
  print('hashCode ints: ${0.hashCode} ${7.hashCode} ${(-7).hashCode}');

  // Limites do Smi (62 bits) e do int de 64 bits.
  const p62 = 4611686018427387904; // 2^62
  final limites = <int>[
    p62 - 1,
    p62,
    -p62,
    -p62 - 1,
    9223372036854775807,
    -9223372036854775807 - 1,
  ];
  for (final v in limites) {
    final c = idInt(v);
    print('$v: identical ${identical(c, v)} ${identical(c + 0, v)} '
        '+1=${c + 1} -1=${c - 1} *2=${c * 2} ~/3=${c ~/ 3} >>1=${c >> 1} '
        'hex=${c.toRadixString(16)} bits=${c.bitLength}');
  }
  final a = idInt(p62 - 1);
  final b = a + 1; // sai do Smi
  final c = b - 1; // volta
  print('atravessa: $b ${identical(b, p62)} ${identical(c, a)} ${b == p62} ${b.hashCode == p62.hashCode}');
  final max = idInt(9223372036854775807);
  print('overflow: ${max + 1} ${(-max - 1) - 1} ${max * max}');
  print('mistura: ${[a, b, c, max].reduce((x, y) => x ^ y)} ${b.compareTo(a)} ${(b / 2)}');

  // Caixas em List<Object>.
  final objs = <Object>[1.5, -0.0, double.nan, double.infinity, p62, 3, 'x', 2.0];
  for (var i = 0; i < 1000; i++) {
    objs.add(i.isEven ? i * 0.5 : p62 + i);
  }
  var somaD = 0.0;
  var somaI = 0;
  for (final o in objs) {
    if (o is double && o.isFinite) somaD += o;
    if (o is int) somaI = (somaI + o) & 0xFFFFFFFFFFFF;
  }
  print('List<Object>: ${objs.length} somaD=$somaD somaI=$somaI');
  print('identical na lista: ${identical(objs[0], 1.5)} ${identical(objs[4], p62)} '
      '${identical(objs[1], -0.0)} ${identical(objs[2], double.nan)}');
  print('objs.indexOf(2.0): ${objs.indexOf(2.0)} indexOf(2): ${objs.indexOf(2)} contains(-0.0): ${objs.contains(0.0)}');

  // Closures que capturam.
  var mutavel = 0;
  var dmut = 0.25;
  final grande = idInt(p62 + 5);
  final fs = <int Function()>[];
  for (var i = 0; i < 5; i++) {
    final d = i * 1.5;
    fs.add(() => i + mutavel + d.toInt() + (grande - p62));
  }
  mutavel = 100;
  print('closures: ${fs.map((f) => f()).toList()}');
  void incr() {
    mutavel++;
    dmut *= 2;
  }

  for (var i = 0; i < 10; i++) {
    incr();
  }
  print('mutável: $mutavel $dmut');
  final geradores = List.generate(3, (k) {
    var estado = k * 1.0;
    return () => estado += 0.5;
  });
  for (var r = 0; r < 3; r++) {
    print('gerador rodada $r: ${geradores.map((g) => g()).toList()}');
  }
  double Function(double) compoe(double Function(double) f, double Function(double) g) => (x) => f(g(x));
  final h = compoe((x) => x * 2, (x) => x + dmut);
  print('composição: ${h(1.0)}');

  // Tear-offs.
  print('estático ==: ${soma == soma} identical: ${identical(soma, soma)}');
  print('estáticos diferentes: ${soma == soma2}');
  print('static de classe: ${Contador.dobro == Contador.dobro} ${identical(Contador.dobro, Contador.dobro)}');
  final ct = Contador(1);
  final t1 = ct.incrementa;
  final t2 = ct.incrementa;
  print('método ==: ${t1 == t2} identical(t1, t1): ${identical(t1, t1)} identical(t1, t2): ${identical(t1, t2)}');
  print('método de outro objeto: ${t1 == Contador(1).incrementa}');
  print('hashCode de tear-offs iguais: ${t1.hashCode == t2.hashCode} ${soma.hashCode == soma.hashCode}');
  final genId = ida;
  print('genérico: ${genId == ida} ${genId(3.5)}');
  int Function(int) instanciada = idGen;
  print('instanciada: ${instanciada == instanciada} ${instanciada(4)}');
  print('closure != closure: ${(() => 1) == (() => 1)}');
  final fechada = () => mutavel;
  print('mesma closure: ${fechada == fechada} ${identical(fechada, fechada)}');

  // Function.apply.
  print('apply posicional: ${Function.apply(soma, [2, 3])}');
  print('apply nomeado: ${Function.apply(nomeado, [1], {#b: 20})}');
  print('apply nomeado 2: ${Function.apply(nomeado, [1], {#c: 'z', #b: 5})}');
  print('apply opcional: ${Function.apply(opcional, [1])} ${Function.apply(opcional, [1, 2, 3.25])}');
  print('apply método: ${Function.apply(ct.incrementa, [10])} ${ct.n}');
  print('apply closure: ${Function.apply((double x, {double y = 0.5}) => x * y, [3.0], {#y: 4.0})}');
  tentar('apply com aridade errada', () => Function.apply(soma, [1]));
  tentar('apply nomeado inexistente', () => Function.apply(nomeado, [1], {#z: 1}));

  // Records.
  final r1 = (1, 2.5, 'três');
  final r2 = (idInt(1), id(2.5), 'tr${'ês'}');
  final r3 = (1, 2.5, 'quatro');
  print('posicional: $r1 == $r2: ${r1 == r2} identical: ${identical(r1, r1)}');
  print('diferentes: ${r1 == r3}');
  if (r1.hashCode == r2.hashCode) print('hashCode igual entre iguais');
  final n1 = (x: 1, y: -0.0, nome: 'a');
  final n2 = (nome: 'a', x: 1, y: id(-0.0));
  print('nomeado: $n1 $n2 ==: ${n1 == n2}');
  if (n1.hashCode == n2.hashCode) print('hashCode nomeado igual entre iguais');
  final nanRec = (double.nan, 1);
  print('record com NaN == si: ${nanRec == nanRec} ${nanRec == (double.nan, 1)}');
  final zeros = (0.0,);
  print('record 0.0 == -0.0: ${zeros == (-0.0,)} ${zeros.$1}');
  final misto = (1, big: p62, d: 1e300 * 10, lista: [1, 2], (a: 1, b: (2, 3)));
  print('misto: $misto');
  print('campos: ${misto.$1} ${misto.big} ${misto.d} ${misto.$2.b.$2}');
  final mapaR = <(int, String), double>{};
  for (var i = 0; i < 1000; i++) {
    mapaR[(i, 'k${i % 7}')] = i * 0.25;
  }
  print('mapa de records: ${mapaR.length} ${mapaR[(999, 'k5')]} ${mapaR[(3, 'k3')]} ${mapaR[(3, 'k4')]}');
  final (p, q) = (grande, dmut);
  final (:x, :nome, y: yy) = n1;
  print('desestruturação: $p $q $x $nome $yy');
  print('runtimeType: ${r1.runtimeType} ${n1.runtimeType} ${() {}.runtimeType}');
}

T idGen<T>(T x) => x;
