// Strings entre isolados no espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md
// §2.12, §5.3): strings pequenas, grandes (região grande) e de dois bytes por
// `SendPort`, `Isolate.run` e `Isolate.exit`; listas e mapas de strings; um
// literal enviado e comparado com `identical` no destino e na volta (a identidade
// de strings montadas em tempo de execução não é conferida: a VM as compartilha
// no grupo de isolados, o DartForge copia).
import 'dart:async';
import 'dart:isolate';

const literal = 'literal compartilhado';

int resumo(String s) {
  var h = 0;
  for (var i = 0; i < s.length; i++) {
    h = (h * 31 + s.codeUnitAt(i)) % 1000000007;
  }
  return h;
}

String descreve(Object? v) {
  if (v is String) return 'String len=${v.length} resumo=${resumo(v)}';
  if (v is List) return 'List(${v.length}) [${v.map(descreve).join('; ')}]';
  if (v is Map) {
    return 'Map(${v.length}) {${v.entries.map((e) => '${e.key}: ${descreve(e.value)}').join('; ')}}';
  }
  return '$v';
}

List<Object?> amostras() {
  final grande = ('grande ' * 200000) + 'fim';
  final grandeDois = ('Āβγ ' * 100000) + '😀';
  final montado = StringBuffer()
    ..write('literal ')
    ..write('compartilhado');
  return [
    '',
    'a',
    'olá',
    '日本語',
    'emoji 😀🎉',
    String.fromCharCode(0xD800), // substituto solto
    grande,
    grandeDois,
    montado.toString(),
    ['x', 'ÿ', 'Ā', literal, grande.substring(0, 10)],
    {'chave': 'valor', 'ção': 'Ωmega', 'lista': ['p', 'q']},
  ];
}

/// Destino que ecoa tudo e responde sobre a identidade do literal.
void eco(SendPort volta) {
  final porta = ReceivePort();
  volta.send(porta.sendPort);
  porta.listen((msg) {
    if (msg == 'fim') {
      porta.close();
      return;
    }
    final m = msg as List;
    final tipo = m[0] as String;
    if (tipo == 'literal') {
      final recebido = m[1] as String;
      volta.send([
        'literal no destino',
        recebido == literal,
        identical(recebido, literal),
        identical(recebido, 'literal compartilhado'),
      ]);
      volta.send(['literal de volta', recebido]);
    } else {
      final valor = m[1];
      volta.send(['eco', descreve(valor), valor]);
    }
  });
}

Future<void> main() async {
  final recebe = ReceivePort();
  await Isolate.spawn(eco, recebe.sendPort);
  final fila = StreamIterator(recebe);
  await fila.moveNext();
  final para = fila.current as SendPort;

  // SendPort: cada amostra vai e volta.
  final lista = amostras();
  for (var i = 0; i < lista.length; i++) {
    para.send(['valor', lista[i]]);
    await fila.moveNext();
    final r = fila.current as List;
    final local = descreve(lista[i]);
    print('amostra $i: ${r[1]}');
    print('  mesmo resumo no destino: ${r[1] == local}');
    print('  volta == original: ${descreve(r[2]) == local}');
  }

  // O literal.
  para.send(['literal', literal]);
  await fila.moveNext();
  print('${fila.current}');
  await fila.moveNext();
  final devolvido = (fila.current as List)[1] as String;
  print('literal de volta ==: ${devolvido == literal} identical: ${identical(devolvido, literal)}');
  para.send('fim');

  // Isolate.run devolvendo strings.
  final r1 = await Isolate.run(() => 'r' * 300000 + 'Ā');
  print('run grande: ${descreve(r1)}');
  final r2 = await Isolate.run(() => ['a', literal, '日本']);
  print('run lista: ${descreve(r2)} identical literal: ${identical(r2[1], literal)}');
  final entrada = 'capturada ${'ç' * 5}';
  final r3 = await Isolate.run(() => entrada.toUpperCase());
  print('run captura: $r3');
  final r4 = await Isolate.run(() => {for (var i = 0; i < 1000; i++) 'k$i': 'v${i * i}'});
  print('run mapa: ${r4.length} ${r4['k999']} ${r4.keys.first} ${resumo(r4.values.join())}');
  final r5 = await Isolate.run(() => literal);
  print('run literal: ${r5 == literal} identical: ${identical(r5, literal)}');

  // Isolate.exit com uma estrutura de strings.
  final saida = ReceivePort();
  await Isolate.spawn((SendPort p) {
    final dados = <String, Object>{
      'texto': 'saída ' * 50000,
      'dois': 'Ω' * 70000,
      'lista': List.generate(2000, (i) => 'e$i'),
      'literal': literal,
    };
    Isolate.exit(p, dados);
  }, saida.sendPort);
  final recebido = await saida.first as Map;
  print('exit: ${(recebido['texto'] as String).length} ${resumo(recebido['texto'] as String)} '
      '${(recebido['dois'] as String).length} ${(recebido['lista'] as List).last} '
      '${recebido['literal'] == literal}');
  print('exit literal identical: ${identical(recebido['literal'], literal)}');

  await fila.cancel();
  print('fim');
}
