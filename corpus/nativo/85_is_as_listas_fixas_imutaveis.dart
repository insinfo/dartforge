// A classe de um valor do runtime lida em linha (`df.classe`, o vetor
// denso `Heap::classes`) e o `is C` pelo mapa de bits (`df.subclasse`):
// listas expansíveis, de tamanho fixo e não modificáveis (`List.filled`
// com `growable: false`, `List.unmodifiable`, `toList`, `const`,
// `List.generate`, `sublist`, `List.of`), strings de um e de dois bytes,
// números, `bool`, records e closures, conferidos por `is`, `as`,
// `runtimeType`, `whereType` e pelas operações que a classe decide
// (`add` numa lista fixa lança). A saída tem de ser a da VM.

String classe(Object? x) {
  final partes = <String>[];
  if (x is List) partes.add('List');
  if (x is List<int>) partes.add('List<int>');
  if (x is List<String>) partes.add('List<String>');
  if (x is Iterable) partes.add('Iterable');
  if (x is Iterable<num>) partes.add('Iterable<num>');
  if (x is String) partes.add('String');
  if (x is Comparable) partes.add('Comparable');
  if (x is Pattern) partes.add('Pattern');
  if (x is num) partes.add('num');
  if (x is int) partes.add('int');
  if (x is double) partes.add('double');
  if (x is bool) partes.add('bool');
  if (x is Record) partes.add('Record');
  if (x is Function) partes.add('Function');
  if (x is Map) partes.add('Map');
  if (x is Set) partes.add('Set');
  if (x is Object) partes.add('Object');
  if (x == null) partes.add('null');
  return partes.join(',');
}

String tentaAdd(List<Object?> l, Object? v) {
  try {
    l.add(v);
    return 'add ok (${l.length})';
  } on UnsupportedError catch (e) {
    return 'UnsupportedError: ${e.message}';
  }
}

String tentaSet(List<Object?> l, Object? v) {
  try {
    l[0] = v;
    return 'set ok';
  } on UnsupportedError catch (e) {
    return 'UnsupportedError: ${e.message}';
  }
}

String tentaLength(List<Object?> l) {
  try {
    l.length = l.length;
    return 'length ok';
  } on UnsupportedError catch (e) {
    return 'UnsupportedError: ${e.message}';
  }
}

List<Object?> amostras() {
  final expansivel = <int>[1, 2, 3];
  final fixa = List<int>.filled(3, 7, growable: false);
  final fixaExpansivel = List<int>.filled(3, 7, growable: true);
  final imutavel = List<int>.unmodifiable([4, 5, 6]);
  final constante = const [8, 9];
  final gerada = List<int>.generate(3, (i) => i * i, growable: false);
  final geradaExpansivel = List<int>.generate(3, (i) => i + 1);
  final copiaFixa = expansivel.toList(growable: false);
  final copia = fixa.toList();
  final fatia = fixa.sublist(1);
  final de = List<int>.of(imutavel, growable: false);
  final vazia = List<String>.empty();
  final vaziaExpansivel = List<String>.empty(growable: true);
  final dinamica = <dynamic>['a', 1, 2.5];
  final textos = ['x', 'y'].toList(growable: false);
  return [
    expansivel, fixa, fixaExpansivel, imutavel, constante, gerada, //
    geradaExpansivel, copiaFixa, copia, fatia, de, vazia, vaziaExpansivel,
    dinamica, textos,
  ];
}

void main() {
  final nomes = [
    'expansivel', 'fixa', 'fixaExpansivel', 'imutavel', 'constante', //
    'gerada', 'geradaExpansivel', 'copiaFixa', 'copia', 'fatia', 'de',
    'vazia', 'vaziaExpansivel', 'dinamica', 'textos',
  ];
  final ls = amostras();
  for (var i = 0; i < ls.length; i++) {
    final l = ls[i] as List<Object?>;
    print('${nomes[i]}: ${classe(l)} | ${l.runtimeType}');
    if (l.isNotEmpty) print('  ${tentaAdd(l, l.first)}');
    if (l.isNotEmpty) print('  ${tentaSet(l, l.first)}');
    print('  ${tentaLength(l)}');
  }

  // `as` que passa e que falha.
  for (final x in ls) {
    try {
      final y = x as List<int>;
      print('as List<int>: ${y.length}');
    } on TypeError {
      print('as List<int>: TypeError');
    }
  }

  // Outros valores do runtime.
  final outros = <Object?>[
    'um byte', 'dois bytes: α', '😀', 42, 1 << 62, -(1 << 63), 3.5, true, //
    (1, 'a'), (x: 1), () => 1, print, null, {'k': 1}, {1, 2}, StringBuffer('b'),
  ];
  for (final x in outros) {
    print('${classe(x)}');
  }

  // `whereType` e `is` no mesmo laço (os mapas de bits já montados).
  final mistura = <Object?>[...ls, ...outros];
  print('listas: ${mistura.whereType<List>().length}');
  print('List<int>: ${mistura.whereType<List<int>>().length}');
  print('strings: ${mistura.whereType<String>().length}');
  print('num: ${mistura.whereType<num>().length}');
  print('Comparable: ${mistura.whereType<Comparable>().length}');
  var n = 0;
  for (var k = 0; k < 2000; k++) {
    for (final x in mistura) {
      if (x is List) n++;
      if (x is Iterable) n += 2;
      if (x is String) n += 3;
      if (x is Comparable) n += 5;
    }
  }
  print('contagem: $n');

  // A mesma lista antes e depois de virar não modificável (uma cópia nova
  // a cada vez: a original continua expansível).
  final base = <int>[1, 2];
  final vista = List<int>.unmodifiable(base);
  base.add(3);
  print('${base.length} ${vista.length} ${tentaAdd(base, 4)} ${tentaAdd(vista, 4)}');
  print('${classe(base)} | ${classe(vista)}');
}
