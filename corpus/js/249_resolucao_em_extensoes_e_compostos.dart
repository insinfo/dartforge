// Resolução de nomes dentro de extensões e escritas por extensão: um setter
// da própria extensão (léxico) ganha do global; um membro do tipo `on` ganha
// de outra extensão; getters, setters e métodos estáticos da extensão, com e
// sem qualificação; `Ext(x).p op= v` e `Ext(x)[i] op= v` passam pela
// extensão explícita na leitura e na escrita; um composto com `?.` no alvo
// para tudo quando o receptor é nulo; `~x` e `x(…)` (o `call`) por extensão;
// uma extensão cujo argumento inferido não respeita o limite não se aplica;
// e uma chamada de construtor por um alias genérico usa os argumentos
// inferidos.
int get g => 1;
set s(int x) => print('top s $x');

class A {
  String get inst => 'A.inst';
  set inst(String v) => print('A.inst= $v');
}

extension Other on A {
  double get other => 2.5;
  set otherSet(double x) => print('Other.otherSet $x');
  double get inst => 9.9;
  set inst(double v) => print('Other.inst= $v');
}

extension MyExt on A {
  set s(int x) => print('MyExt.s $x');
  int m() => 7;

  static int get sg => 3;
  static set ss(int x) => print('MyExt.ss $x');
  static int sm() => 4;
  static int field = 10;

  void test() {
    print(g);
    s = 3;
    print(inst);
    inst = 'x';
    print(other);
    otherSet = 1.5;
    print(m());
    print(sg);
    ss = 5;
    print(sm());
    field = 11;
    print(field);
    print(MyExt.sg);
    MyExt.ss = 6;
  }
}

class C {
  int operator [](int i) => i;
  void operator []=(int i, int v) => print('C[]= $i $v');
  int get p => 1;
  set p(int v) => print('C.p= $v');
}

extension E on C {
  int operator [](int i) => i * 10;
  void operator []=(int i, int v) => print('E[]= $i $v');
  int get p => 2;
  set p(int v) => print('E.p= $v');
}

class N {
  int p = 1;
}

extension Ops on Object {
  String operator ~() => '(~$this)';
  String call(Object a) => '$this($a)';
}

extension Bounded<T extends num> on List<T> {
  int get size => 17;
}

extension Any on Object {
  int get size => 0;
}

class Box<X> {
  Box(Type t) {
    print('$X $t');
  }
}

typedef Alias<X> = Box<X>;

void main() {
  A().test();

  final c = C();
  print(E(c)[1]);
  E(c)[2] = 3;
  E(c)[4] += 5;
  E(c)[6]++;
  print(E(c).p);
  E(c).p = 7;
  E(c).p += 8;
  c[1] += 1;
  c.p += 1;

  N? n;
  print(n?.p += 1);
  print(n?.p++);
  n = N();
  print(n?.p += 1);

  Object o = 'a';
  print(~o);
  print(o('b'));

  List<Object> objects = <int>[];
  List<int> ints = [];
  print([objects.size, ints.size]);

  Alias<num> a1 = Alias(num);
  Alias(int);
  print(a1.runtimeType);
}
