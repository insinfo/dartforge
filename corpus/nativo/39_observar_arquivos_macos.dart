// N02 no macOS: `FileSystemEntity.watch` pelo FSEvents, como a VM
// (`file_system_watcher_macos.cc`): criação, modificação, mudança de nome
// (o FSEvents dá um evento para cada ponta; o que ainda existe vira criação
// e o que não existe, remoção), subdiretório, remoção, observação recursiva
// e não recursiva, filtro de eventos e a remoção do próprio diretório, que
// encerra o fluxo. O FSEvents junta eventos próximos e repete bandeiras de
// um mesmo arquivo: cada passo imprime só se o evento esperado chegou.
import 'dart:async';
import 'dart:io';

String nome(Directory base, String caminho) =>
    caminho == base.path ? '.' : caminho.substring(base.path.length + 1);

String descrever(Directory base, FileSystemEvent e) {
  final p = nome(base, e.path);
  return switch (e) {
    FileSystemCreateEvent() => 'criado $p dir=${e.isDirectory}',
    FileSystemModifyEvent(:final contentChanged) => 'modificado $p conteudo=$contentChanged',
    FileSystemDeleteEvent() => 'apagado $p',
    FileSystemMoveEvent(:final destination) => 'movido $p -> ${destination == null ? null : nome(base, destination)}',
  };
}

/// Faz `acao` e espera (até 3 s) que `vistos` tenha todos os `esperados`;
/// imprime quais chegaram.
Future<void> passo(List<String> vistos, String titulo, void Function() acao, List<String> esperados) async {
  final inicio = vistos.length;
  acao();
  bool todos() => esperados.every(vistos.skip(inicio).contains);
  for (var i = 0; i < 300 && !todos(); i++) {
    await Future.delayed(const Duration(milliseconds: 10));
  }
  await Future.delayed(const Duration(milliseconds: 150));
  final novos = vistos.skip(inicio).toSet();
  print('$titulo: ${[for (final e in esperados) '$e=${novos.contains(e)}'].join(', ')}');
}

Future<void> main() async {
  if (!Platform.isMacOS) {
    print('observação comparada só no macOS');
    return;
  }
  print('suportado: ${FileSystemEntity.isWatchSupported}');
  // O caminho real: o FSEvents entrega `/private/var/…` para `/var/…`.
  final base = Directory(Directory.systemTemp.createTempSync('df_obs').resolveSymbolicLinksSync());
  final vistos = <String>[];
  final recursivos = <String>[];
  final fim = Completer<void>();
  final sub = base.watch().listen((e) => vistos.add(descrever(base, e)), onDone: fim.complete);
  final subR = base.watch(recursive: true).listen((e) => recursivos.add(descrever(base, e)));
  await Future.delayed(const Duration(milliseconds: 200));

  final a = File('${base.path}/a.txt');
  await passo(vistos, 'criar', () => a.writeAsStringSync('um'), ['criado a.txt dir=false']);
  await passo(vistos, 'modificar', () => a.writeAsStringSync('dois', mode: FileMode.append),
      ['modificado a.txt conteudo=true']);
  await passo(vistos, 'renomear', () => a.renameSync('${base.path}/b.txt'), ['apagado a.txt', 'criado b.txt dir=false']);
  await passo(vistos, 'subdiretório', () => Directory('${base.path}/d').createSync(), ['criado d dir=true']);
  await passo(recursivos, 'recursivo', () => File('${base.path}/d/x.txt').writeAsStringSync('x'),
      ['criado d/x.txt dir=false']);
  print('não recursivo vê d/x.txt: ${vistos.any((s) => s.contains('d/x.txt'))}');
  await passo(vistos, 'apagar', () => File('${base.path}/b.txt').deleteSync(), ['apagado b.txt']);
  await subR.cancel();

  // Só os eventos pedidos: um observador de criações não vê a remoção.
  final outro = Directory('${base.path}/o')..createSync();
  await Future.delayed(const Duration(milliseconds: 200));
  final criacoes = <String>[];
  final sub2 = outro.watch(events: FileSystemEvent.create).listen((e) => criacoes.add(descrever(outro, e)));
  await Future.delayed(const Duration(milliseconds: 200));
  await passo(criacoes, 'filtro', () {
    File('${outro.path}/c.txt').createSync();
    File('${outro.path}/c.txt').deleteSync();
  }, ['criado c.txt dir=false']);
  print('filtro vê remoção: ${criacoes.any((s) => s.startsWith('apagado'))}');
  await sub2.cancel();

  // Apagar o próprio diretório encerra o fluxo.
  base.deleteSync(recursive: true);
  var terminou = true;
  await fim.future.timeout(const Duration(seconds: 3), onTimeout: () => terminou = false);
  print('fluxo terminou: $terminou; apagado .: ${vistos.contains('apagado .')}');
  await sub.cancel();
}
