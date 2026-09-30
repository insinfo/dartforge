// A classe e as marcas "fixa"/"imutável" de cada slot do heap
// (`Heap::classes`) sob coleta: centenas de milhares de listas de tamanho
// fixo, não modificáveis e expansíveis que morrem e cujos slots são
// reusados por valores de outras classes (a marca não pode ficar no slot
// reusado), campos `late` de objetos jovens que morrem (a marca de
// inicializado sai na coleta menor, e o bloco reusado lê o campo como não
// inicializado) e o `is` depois de cada rodada. Também num isolado novo
// (`Isolate.run`: outro heap, outro contexto). A saída tem de ser a da VM.
import 'dart:isolate';

class ComLate {
  late int valor;
  late final String nome = 'n$valor';
  int? outro;
}

String lerLate(ComLate c) {
  try {
    return '${c.valor}';
  } on Error catch (e) {
    return e.runtimeType.toString().contains('Late') ? 'não inicializado' : 'erro';
  }
}

String tipoDe(Object? x) {
  if (x is List) {
    try {
      x.add(x.first);
      x.removeLast();
      return 'expansível';
    } on UnsupportedError {
      try {
        x[0] = x[0];
        return 'fixa';
      } on UnsupportedError {
        return 'imutável';
      }
    }
  }
  if (x is String) return 'texto';
  if (x is Map) return 'mapa';
  if (x is double) return 'double';
  if (x is int) return 'int';
  return 'outro';
}

int rodada(int semente) {
  var soma = 0;
  final vivos = <Object?>[];
  for (var i = 0; i < 60000; i++) {
    final k = (i * 7 + semente) % 6;
    final Object? v = switch (k) {
      0 => List<int?>.filled(3, i, growable: false),
      1 => List<int?>.unmodifiable([i, i + 1]),
      2 => <int?>[i],
      3 => 'texto $i',
      4 => {'k': i},
      _ => i * 0.5,
    };
    if (i % 997 == 0) vivos.add(v);
    if (v is List && v.length == 3) soma++;
    if (v is String) soma += 2;
  }
  // Os que sobreviveram continuam com a classe deles.
  final contagem = <String, int>{};
  for (final v in vivos) {
    final t = tipoDe(v);
    contagem[t] = (contagem[t] ?? 0) + 1;
  }
  final chaves = contagem.keys.toList()..sort();
  print('rodada $semente: soma $soma, ${[for (final c in chaves) '$c=${contagem[c]}'].join(' ')}');
  return soma;
}

int lates() {
  var inicializados = 0;
  var naoInicializados = 0;
  ComLate? guardado;
  for (var i = 0; i < 40000; i++) {
    final c = ComLate();
    if (i % 3 == 0) c.valor = i;
    if (lerLate(c) == 'não inicializado') {
      naoInicializados++;
    } else {
      inicializados++;
    }
    if (i == 30001) guardado = c;
  }
  print('late: $inicializados inicializados, $naoInicializados não');
  if (guardado != null) {
    print('guardado: ${lerLate(guardado)}');
    guardado.valor = 5;
    print('guardado: ${lerLate(guardado)} ${guardado.nome}');
  }
  return inicializados;
}

Future<void> main() async {
  var total = 0;
  for (var s = 0; s < 4; s++) {
    total += rodada(s);
  }
  print('total $total');
  lates();

  // Listas que viram fixas por `toList(growable: false)` depois de muitas
  // mortes: as expansíveis novas que reusam os slots continuam expansíveis.
  final fixas = [for (var i = 0; i < 1000; i++) [i].toList(growable: false)];
  fixas.clear();
  for (var i = 0; i < 100000; i++) {
    [i, i].toList(growable: false);
  }
  final novas = [for (var i = 0; i < 1000; i++) <int>[i]];
  var ok = 0;
  for (final l in novas) {
    l.add(1);
    if (l.length == 2) ok++;
  }
  print('novas expansíveis: $ok');

  // Outro isolado: outro heap e outro contexto, a mesma resposta.
  final r = await Isolate.run(() {
    var s = 0;
    for (var i = 0; i < 20000; i++) {
      final Object l = i.isEven ? List<int>.filled(1, i, growable: false) : List<int>.unmodifiable([i]);
      if (l is List<int>) s++;
      s += tipoDe(l) == 'fixa' ? 1 : 0;
    }
    return '$s ${tipoDe(const [1])} ${tipoDe([1].toList(growable: false))} ${tipoDe(<int>[3])}';
  });
  print('isolado: $r');
}
