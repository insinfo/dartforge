// `add` em linha de referências: a lista de tipo exato recebe sem checagem; a
// lista covariante (vista por um supertipo) continua lançando `TypeError`, e o
// crescimento em código gerado preserva os elementos, o tipo e a identidade.

class Ponto {
  final int x;
  Ponto(this.x);
  @override
  String toString() => 'P$x';
}

void adiciona(List<Object> l, Object v) {
  try {
    l.add(v);
    print('ok ${l.length}');
  } on TypeError catch (e) {
    print('TypeError ${e.runtimeType}');
  }
}

List<String> nomes(int n) {
  final l = <String>[];
  for (var i = 0; i < n; i++) {
    l.add('n$i');
  }
  return l;
}

void main() {
  // Crescimento de 0 a 40 elementos: 3, 7, 15, 31, 63.
  final s = nomes(40);
  print('${s.length} ${s.first} ${s.last} ${s.runtimeType}');
  final p = <Ponto>[];
  for (var i = 0; i < 20; i++) {
    p.add(Ponto(i));
  }
  print('${p.length} ${p[0]} ${p[19]}');
  // Covariância: a mesma lista, vista como List<Object>.
  adiciona(<String>['a'], 'b');
  adiciona(<String>['a'], 1);
  adiciona(<Ponto>[], Ponto(1));
  adiciona(<Ponto>[], 'x');
  final List<Object?> o = <int?>[];
  try {
    o.add('x');
  } on TypeError {
    print('TypeError int?');
  }
  o.add(null);
  o.add(3);
  print(o);
  // Lista de Object recebe qualquer coisa.
  final all = <Object>[];
  for (var i = 0; i < 10; i++) {
    all.add(i.isEven ? i : 'v$i');
  }
  print(all);
  // Lista fixa e não modificável: o caminho usual lança.
  final fixa = List<String>.filled(2, 'z');
  try {
    fixa.add('w');
  } on UnsupportedError catch (e) {
    print('fixa: ${e.message}');
  }
  final imut = List<String>.unmodifiable(['q']);
  try {
    imut.add('w');
  } on UnsupportedError catch (e) {
    print('imut: ${e.message}');
  }
  // Muitas listas pequenas (coleta jovem durante o crescimento).
  var total = 0;
  for (var k = 0; k < 20000; k++) {
    final l = <String>[];
    for (var j = 0; j < 9; j++) {
      l.add('e$j');
    }
    total += l.length + l[8].length;
  }
  print(total);
}
