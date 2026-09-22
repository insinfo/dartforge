// for clássico (múltiplas variáveis, partes vazias) e for-in sobre List/Set/Map/Iterable/runes/records.
Iterable<int> pares(int n) => Iterable.generate(n, (i) => i * 2);

void main() {
  // for com múltiplas variáveis
  for (var i = 0, j = 10; i < j; i++, j--) {
    print('i=$i j=$j');
  }

  // for sem inicialização
  var k = 0;
  for (; k < 3; k++) {
    print('k=$k');
  }

  // for sem incremento
  for (var m = 0; m < 3;) {
    print('m=$m');
    m++;
  }

  // for sem nenhuma parte
  var z = 0;
  for (;;) {
    z++;
    if (z >= 3) break;
  }
  print('z=$z');

  // for com passo diferente e decrescente
  for (var i = 10; i > 0; i -= 3) {
    print('desce $i');
  }

  // for-in sobre List
  for (final x in [1, 2, 3]) {
    print('list $x');
  }

  // for-in sobre Set (ordem de inserção)
  for (final s in {'c', 'a', 'b'}) {
    print('set $s');
  }

  // for-in sobre Map.entries e keys/values
  final mapa = {'um': 1, 'dois': 2, 'três': 3};
  for (final e in mapa.entries) {
    print('${e.key}=${e.value}');
  }
  for (final chave in mapa.keys) {
    print('chave $chave');
  }
  for (final v in mapa.values) {
    print('valor $v');
  }

  // for-in sobre Iterable preguiçoso
  for (final p in pares(4)) {
    print('par $p');
  }

  // for-in sobre String.runes e codeUnits
  for (final r in 'aé'.runes) {
    print('rune $r');
  }
  for (final cu in 'ab'.codeUnits) {
    print('codeUnit $cu');
  }

  // for-in sobre split
  for (final palavra in 'x y z'.split(' ')) {
    print('palavra $palavra');
  }

  // for-in com desestruturação de records
  final pontos = [(1, 2), (3, 4), (5, 6)];
  for (final (x, y) in pontos) {
    print('ponto $x,$y soma ${x + y}');
  }
  final nomeados = [(nome: 'a', idade: 1), (nome: 'b', idade: 2)];
  for (final (:nome, :idade) in nomeados) {
    print('$nome tem $idade');
  }

  // for-in com MapEntry desestruturado
  for (final MapEntry(:key, :value) in mapa.entries) {
    print('$key -> $value');
  }

  // for-in com var e modificação local
  for (var n in [1, 2]) {
    n *= 10;
    print('n=$n');
  }

  // for-in com índice via indexed
  for (final (i, v) in ['a', 'b', 'c'].indexed) {
    print('$i:$v');
  }

  // for-in sobre lista vazia
  for (final _ in <int>[]) {
    print('nunca');
  }

  // for aninhado
  var total = 0;
  for (var i = 1; i <= 3; i++) {
    for (var j = 1; j <= 3; j++) {
      total += i * j;
    }
  }
  print('total $total');
}
