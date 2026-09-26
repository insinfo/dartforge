// A validade do endereço dos elementos no acesso direto às listas
// (`lower/tipados.rs`): o buffer de uma lista pode ser realocado por
// qualquer chamada que a faça crescer — por outra referência, por um
// callback, no meio de uma expressão — e o código gerado tem de reobter o
// endereço depois dela. Também: coleta entre a obtenção e o uso, e
// gravações alternadas pelo acesso direto e pelos métodos do SDK.
import 'dart:typed_data';

List<int> global = <int>[];

int cresceEDevolve(List<int> l, int v) {
  for (var k = 0; k < 100; k++) {
    l.add(k);
  }
  return v;
}

int lixo() {
  // Aloca bastante: dispara coletas (e o GC stress coleta em todas).
  var s = 0;
  for (var k = 0; k < 200; k++) {
    s += List<int>.filled(50, k).length + '$k'.length;
  }
  return s;
}

void main() {
  // Crescimento por outra referência entre leituras do mesmo laço.
  final a = <int>[1, 2, 3];
  final alias = a;
  var s = 0;
  for (var i = 0; i < a.length && i < 400; i++) {
    s += a[i];
    if (i % 3 == 0) alias.addAll(List<int>.filled(10, i));
  }
  print('alias: $s ${a.length}');

  // A realocação no meio de uma expressão: a leitura da direita cresce a
  // lista antes da gravação.
  final b = <int>[5];
  b[0] = cresceEDevolve(b, 7) + b[0];
  print('meio da expressão: ${b[0]} ${b.length} ${b[100]}');
  final c = <int>[1, 2];
  c[1] += cresceEDevolve(c, 3);
  print('composto: ${c[1]} ${c.length}');

  // Callback que muda o comprimento durante a iteração por índice.
  final d = <double>[0.5, 1.5, 2.5];
  final cortar = () => d.length = 1;
  var soma = 0.0;
  for (var i = 0; i < d.length; i++) {
    soma += d[i];
    if (i == 0) cortar();
  }
  print('callback: $soma ${d.length}');

  // Global alterada por função chamada no laço.
  global = List<int>.generate(5, (i) => i);
  var t = 0;
  for (var i = 0; i < global.length; i++) {
    t += global[i];
    if (i == 2) global = <int>[100, 200, 300, 400, 500, 600];
  }
  print('global trocada: $t');

  // Coleta entre o endereço e o uso.
  final e = List<int>.generate(20, (i) => i * i);
  var u = 0;
  for (var i = 0; i < e.length; i++) {
    u += e[i] + (i % 5 == 0 ? lixo() : 0) - (i % 5 == 0 ? lixo() : 0);
    e[i] = u;
  }
  print('coleta: $u ${e[19]} ${e.last}');

  // Gravações alternadas: acesso direto, métodos do SDK e dynamic.
  final f = List<int>.filled(8, 0);
  dynamic g = f;
  for (var i = 0; i < f.length; i++) {
    if (i.isEven) {
      f[i] = i * 10;
    } else {
      g[i] = i * 100;
    }
  }
  f.setRange(2, 4, [7, 8]);
  f[5] += f[2];
  print('alternadas: $f ${f.fold<int>(0, (x, y) => x + y)} ${g[5]}');
  final h = <double>[1.0, 2.0];
  h.add(3.0);
  h[2] = h[0] + h[1];
  (h as dynamic)[0] = 9.5;
  print('double alternadas: $h ${h.reduce((x, y) => x + y)}');

  // Elemento guardado como int grande e como caixa (via Object).
  final big = <int>[1 << 62, -(1 << 62)];
  final objs = <Object>[1, 2.5];
  big[0] += 1;
  print('grandes: ${big[0]} ${big[1]} ${objs[0]} ${objs[1]}');

  // Lista tipada: visão e escrita pela lista base alternadas com a leitura
  // direta.
  final base = Int32List(8);
  final v = Int32List.sublistView(base, 2, 6);
  for (var i = 0; i < v.length; i++) {
    v[i] = i + 1;
    base[i] += 10;
  }
  print('tipadas: $base $v');
}
