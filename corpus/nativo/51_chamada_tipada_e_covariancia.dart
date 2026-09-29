// Chamada tipada e entrada que só confere os parâmetros covariantes
// (`crates/emit_native/src/lower/entrada_tipada.rs`): o receptor de tipo
// estático conhecido dispensa a conferência dos parâmetros que o analisador
// já garantiu, mas não a dos covariantes (genérico da classe, `covariant`
// escrito ou herdado), e o argumento `dynamic` ganha o cast implícito no
// ponto de chamada, logo depois de avaliado (o do CFE, sem " of 'nome'").
import 'dart:collection';

void tente(String rotulo, void Function() f) {
  try {
    f();
    print('$rotulo: ok');
  } catch (e) {
    print('$rotulo: ${e.runtimeType} $e');
  }
}

class A {
  void m(num x) => print('A.m $x');
  void n({required num x}) => print('A.n $x');
}

class B extends A {
  @override
  void m(covariant int x) => print('B.m $x');
  @override
  void n({required covariant int x}) => print('B.n $x');
}

// Herda a covariância de `B.m` sem escrever a palavra.
class C extends B {
  @override
  void m(int x) => print('C.m $x');
}

abstract class Caixa<T> {
  void por(T valor);
  void porTodos(Iterable<T> valores);
  void cada(void Function(T) f);
}

class CaixaDeInt extends Caixa<int> {
  final itens = <int>[];
  @override
  void por(int valor) => itens.add(valor);
  @override
  void porTodos(Iterable<int> valores) => itens.addAll(valores);
  @override
  void cada(void Function(int) f) => itens.forEach(f);
}

// Sobrescrita que alarga o parâmetro: nada a conferir.
class Larga extends Caixa<int> {
  @override
  void por(Object? valor) => print('Larga.por $valor');
  @override
  void porTodos(Iterable<Object?> valores) => print('Larga.porTodos $valores');
  @override
  void cada(void Function(int) f) => f(7);
}

class MinhaLista extends ListBase<int> {
  final _l = <int>[];
  @override
  int get length => _l.length;
  @override
  set length(int n) => _l.length = n;
  @override
  int operator [](int i) => _l[i];
  @override
  void operator []=(int i, int v) => _l[i] = v;
  @override
  void add(int v) => _l.add(v);
}

void recebeIterable(Iterable<int> x) => print('recebeIterable $x');
void recebeTexto(String s) => print('recebeTexto $s');
void recebeInt(int i) => print('recebeInt $i');
void recebeNomeado({required String s, int n = 0}) => print('recebeNomeado $s $n');
void recebeFuncao(int Function(int) f) => print('recebeFuncao ${f(2)}');
void recebeDois(int a, String b) => print('recebeDois $a $b');

class P {
  final Iterable<int> x;
  P(this.x);
  factory P.fabrica(Iterable<int> x) => P(x);
  P.nomeado({required Iterable<int> this.x});
}

class G<T> {
  final List<T> xs;
  G(this.xs);
}

int efeitos = 0;
String efeito() {
  efeitos++;
  return 'efeito';
}

void main() {
  // Covariância das classes genéricas pelo receptor tipado.
  List<Object?> lo = <int>[1, 2];
  tente('lista add', () => lo.add('x'));
  tente('lista []=', () => lo[0] = 'x');
  tente('lista []= ok', () => lo[0] = 5);
  tente('lista insert', () => lo.insert(0, 1.5));
  tente('lista addAll', () => lo.addAll(<Object>['a']));
  print(lo);
  Map<Object, Object?> mo = <String, int>{};
  tente('mapa chave', () => mo[1] = 2);
  tente('mapa valor', () => mo['a'] = 'b');
  tente('mapa ok', () => mo['a'] = 3);
  tente('mapa putIfAbsent', () => mo.putIfAbsent('b', () => 'x'));
  print(mo);
  Set<Object> so = <String>{};
  tente('conjunto', () => so.add(1));
  tente('conjunto ok', () => so.add('um'));
  print(so);

  // `covariant` escrito e herdado.
  A a = B();
  tente('covariant escrito', () => a.m(1.5));
  tente('covariant escrito ok', () => a.m(3));
  tente('covariant nomeado', () => a.n(x: 2.5));
  a = C();
  tente('covariant herdado', () => a.m(2.5));
  tente('covariant herdado ok', () => a.m(4));

  // Genérico da classe em posição de parâmetro, pela interface.
  Caixa<Object> caixa = CaixaDeInt();
  tente('caixa por', () => caixa.por('x'));
  tente('caixa porTodos', () => caixa.porTodos(<Object>['y']));
  tente('caixa cada', () => caixa.cada((Object o) => print('cada $o')));
  tente('caixa por ok', () => caixa.por(9));
  caixa = Larga();
  tente('larga por', () => caixa.por('x'));
  tente('larga porTodos', () => caixa.porTodos(<Object>['y']));

  // Classe do programa que implementa uma interface do SDK.
  List<num> ml = MinhaLista();
  tente('minha lista add', () => ml.add(1.5));
  tente('minha lista []=', () => ml[0] = 2.5);
  tente('minha lista ok', () => ml.add(3));
  print(ml);

  // Argumentos `dynamic`: o cast implícito no ponto de chamada.
  dynamic nulos = <int?>[null];
  dynamic texto = 'abc';
  dynamic numero = 1.5;
  dynamic inteiro = 2;
  dynamic nulo;
  tente('funcao Iterable<int>', () => recebeIterable(nulos));
  tente('funcao Iterable<int> ok', () => recebeIterable(<int>[1] as dynamic));
  tente('funcao String', () => recebeTexto(numero));
  tente('funcao int', () => recebeInt(texto));
  tente('funcao int nulo', () => recebeInt(nulo));
  tente('funcao nomeado', () => recebeNomeado(s: inteiro));
  tente('funcao nomeado 2', () => recebeNomeado(s: 'x', n: texto));
  tente('funcao de funcao', () => recebeFuncao(((String s) => 1) as dynamic));
  dynamic ff = (String s) => 1;
  tente('funcao de funcao 2', () => recebeFuncao(ff));
  tente('construtor', () => P(nulos));
  tente('fabrica', () => P.fabrica(nulos));
  tente('construtor nomeado', () => P.nomeado(x: nulos));
  tente('construtor generico', () => G<int>(nulos));
  tente('construtor generico ok', () => print(G<int?>(nulos).xs));
  tente('ordem', () => recebeDois(texto, efeito()));
  print('efeitos $efeitos');
  tente('ordem 2', () => recebeDois(inteiro, efeito()));
  print('efeitos $efeitos');

  // Membros do SDK com receptor tipado e argumento `dynamic`.
  String s = 'abcdef';
  tente('substring', () => print(s.substring(numero)));
  tente('substring ok', () => print(s.substring(inteiro)));
  tente('codeUnitAt', () => print(s.codeUnitAt(texto)));
  tente('indexOf', () => print(s.indexOf(inteiro)));
  List<int> li = [10, 20, 30];
  tente('lista indice', () => print(li[texto]));
  tente('lista indice ok', () => print(li[inteiro]));
  tente('lista gravar indice', () => li[numero] = 1);
  tente('lista gravar valor', () => li[0] = texto);
  tente('lista add dynamic', () => li.add(texto));
  tente('lista sublist', () => print(li.sublist(numero)));
  StringBuffer sb = StringBuffer();
  tente('buffer', () => sb.writeCharCode(texto));
  Map<String, int> msi = {};
  tente('mapa dynamic', () => msi[inteiro] = 1);
  tente('mapa dynamic valor', () => msi['a'] = texto);
  num n = 7;
  tente('num +', () => print(n + inteiro));
  tente('num + texto', () => print(n + texto));
  tente('int ~/ dynamic', () => print(n ~/ numero));

  // `for-in` com o elemento `dynamic` numa variável tipada.
  tente('for-in', () {
    for (int x in (<Object>[1, 'a'] as dynamic)) {
      print('for-in $x');
    }
  });
  tente('for-in Iterable cru', () {
    Iterable cru = <Object>[2, 3.5];
    for (int x in cru) {
      print('for-in cru $x');
    }
  });
  tente('for-in variavel existente', () {
    int y = 0;
    for (y in (<Object>[4, 'b'] as dynamic)) {
      print('for-in y $y');
    }
  });
  tente('List.from', () => List<int>.from(<Object>[1, 'a']));
  tente('List.from ok', () => print(List<int>.from(<Object>[1, 2])));
  tente('List.of', () => print(List<num>.of(<int>[1, 2])));

  // Chamada dinâmica: a entrada que confere tudo, com o nome do parâmetro.
  dynamic dl = <int>[1];
  tente('dinamica add', () => dl.add('x'));
  tente('dinamica aridade', () => dl.add(1, 2));
  tente('dinamica nomeado', () => dl.add(1, x: 2));
  dynamic ds = 'abc';
  tente('dinamica substring', () => print(ds.substring('x')));
  dynamic corte = s.substring;
  tente('tear-off', () => print(corte(texto)));
  tente('tear-off ok', () => print(corte(4)));
  dynamic metodo = a.m;
  tente('tear-off covariante', () => metodo(1.5));
}
