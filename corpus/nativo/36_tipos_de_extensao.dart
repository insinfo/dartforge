// Tipos de extensão apagados no backend nativo: membros com a representação
// como receptor (também async, sync* e closures), genéricos (argumentos do
// tipo estático do receptor, parâmetro fantasma, implements genérico),
// tear-offs e suas assinaturas, padrões de objeto, cascata, extensão sobre
// tipo de extensão, constantes, is/as e runtimeType apagados.
import 'dart:async';

extension type Id(int v) {
  int get length => 99;
  bool operator <(Id o) => v < o.v;
  Future<int> dobroDepois() async {
    await null;
    return v * 2;
  }

  Iterable<int> ate() sync* {
    for (var i = 0; i < v; i++) yield i;
  }

  int Function() fecho() => () => v + 1;
}

extension type Lista<T>(List<T> l) {
  int get length => -1;
  T operator [](int i) => l[l.length - 1 - i];
  List<T> copia() => [...l];
  R reduzir<R>(R inicial, R Function(R, T) f) {
    var r = inicial;
    for (var x in l) r = f(r, x);
    return r;
  }
}

extension type Fantasma<T>(int n) {
  List<T> vazia() => <T>[];
  bool eh(Object? o) => o is T;
}

extension type Base<T>(List<T> l) {
  T get primeiro => l.first;
  List<T> nova() => <T>[];
}

extension type Filho<U>(List<U> l) implements Base<U> {
  U get ultimo => l.last;
}

extension type const Cor(int rgb) {
  static const vermelho = Cor(0xff0000);
  static const azul = Cor(0x0000ff);
}

class Caixa {
  final Id id;
  Caixa(this.id);
}

String descreve(Object? o) => switch (o) {
      int n when n > 10 => 'grande $n',
      int n => 'int $n',
      _ => 'outro',
    };

extension type Num(int v) implements Object {
  int soma(int a) => v + a;
  int get dobro => v * 2;
  set total(int x) => print('total $x');
}

extension type Pacote<T>(List<T> itens) implements Object {
  T primeiro() => itens.first;
  void Function(T) adicionador() => (T x) => itens.add(x);
  List<T> Function() fabrica() => () => <T>[];
}

extension Dobrar on Num {
  int get quadruplo => dobro * 2;
}

extension ExtPacote<T> on Pacote<T> {
  int get tamanho => itens.length;
  List<T> vazia() => <T>[];
}

void main() async {
  var a = Id(3);
  print(a.length);
  print(a < Id(4));
  print(await a.dobroDepois());
  print(a.ate().toList());
  print(a.fecho()());
  var l = Lista<int>([1, 2, 3]);
  print(l.length);
  print(l[0]);
  print(l.copia().runtimeType);
  print(l.reduzir<String>('', (r, x) => '$r$x'));
  var f = Fantasma<String>(1);
  print(f.vazia().runtimeType);
  print(f.eh('x'));
  print(f.eh(1));
  var fi = Filho<double>([1.5, 2.5]);
  print(fi.primeiro);
  print(fi.ultimo);
  print(fi.nova().runtimeType);
  Id? nada;
  print(nada?.length);
  print(nada == null);
  print(identical(Cor.vermelho, Cor(0xff0000)));
  print(const [Cor.vermelho, Cor.azul]);
  print(Cor.azul.rgb);
  print(Caixa(Id(7)).id.v);
  print(descreve(Id(20)));
  print(descreve(Id(2)));
  Object? o = Id(5);
  print(o is Id);
  print(o is Lista<int>);
  print(<Id>[Id(1)] is List<int>);
  print((o as Id).v);
  var fs = [Id(1), Id(2)];
  print(fs.map((x) => x.length).toList());
  print(fs.runtimeType);
  var m = {Id(1): 'um'};
  print(m[Id(1)]);
  print(Id(4).hashCode == 4.hashCode);
  print(Id(4).toString());
  var t = a.ate;
  print(t().length);
  final Map<String, Id> mm = {'x': Id(9)};
  print(mm['x']!.v + 1);

  segundaParte();
}

void segundaParte() {
  var a = Num(3);
  var s = a.soma;
  print(s.runtimeType);
  print(s(4));
  var c = Pacote<String>(['a']);
  var p = c.primeiro;
  print(p.runtimeType);
  var ad = c.adicionador();
  ad('b');
  print(c.itens);
  print(ad.runtimeType);
  print(c.fabrica()().runtimeType);
  print(a.quadruplo);
  print(c.tamanho);
  print(c.vazia().runtimeType);
  a
    ..total = 5
    ..soma(1);
  Object o = Num(8);
  switch (o) {
    case Num(dobro: var d):
      print('dobro $d');
  }
  if (o case Num(v: var x) when x > 3) print('v $x');
  print(Num.new.runtimeType);
  var construtores = [Num.new, Num.new];
  print(construtores.map((f) => f(2).v).toList());
  Object g = Pacote<int>([4, 5]);
  if (g case Pacote<int>(itens: var it)) print(it);
}
