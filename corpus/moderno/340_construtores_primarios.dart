// requer-dart: 3.13
// Construtores primários (3.13): `var`/`final` declaram campo; parâmetro
// simples não; parte `this : inits { corpo }`; nome `C.id`; `const`; enum;
// nomeados, opcionais e defaults. (Extension type: 347.)
class Point(var int x, var int y);

class Nome(String cru) {
  final String up = cru.toUpperCase();
  final int tamanho = cru.length;
}

class Log(final String tag, [int n = 1]) {
  this : assert(n > 0) {
    print('corpo: $tag $n');
  }
}

class const K(final int v);

class Opc({required final int id, var String? nome, final String tipo = 'padrão'});

class Rotulado.criar(final String rotulo) {
  String get texto => '<$rotulo>';
}

class Pai(final int base);

class Filho(super.base, final int extra) extends Pai {
  int get soma => base + extra;
}

enum E(final String rot) {
  a('A'),
  b('B');

  String get dupla => rot * 2;
}

void main() {
  var p = Point(1, 2);
  p.x = 5;
  print('${p.x} ${p.y}');
  var n = Nome('abc');
  print('${n.up} ${n.tamanho}');
  Log('t');
  Log('u', 3);
  print(identical(const K(1), const K(1)));
  print(const K(2).v);
  var o = Opc(id: 1, nome: 'x');
  o.nome = 'y';
  print('${o.id} ${o.nome} ${o.tipo}');
  print(Opc(id: 2).nome);
  print(Rotulado.criar('r').texto);
  print(Filho(10, 5).soma);
  print(E.b.rot);
  print(E.a.dupla);
}
