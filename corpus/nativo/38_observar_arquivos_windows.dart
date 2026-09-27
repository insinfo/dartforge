// N02 no Windows: `FileSystemEntity.watch` pelo `ReadDirectoryChangesW` na
// porta de conclusão, como a VM (`file_system_watcher_win.cc`): criação,
// modificação, mudança de nome (o par velho/novo que o Dart junta),
// subdiretório (o "é diretório" vem do disco), remoção, filtro de eventos,
// observação recursiva, um arquivo no lugar do diretório, cancelamento e
// caminho inexistente. O sistema pode juntar ou repetir notificações de uma
// mesma escrita: cada passo imprime o conjunto dos eventos vistos.
import 'dart:async';
import 'dart:io';

String nome(Directory base, String caminho) => caminho.substring(base.path.length + 1);

String descrever(Directory base, FileSystemEvent e) {
  final p = e.path == base.path ? '.' : nome(base, e.path);
  return switch (e) {
    FileSystemCreateEvent() => 'criado $p dir=${e.isDirectory}',
    FileSystemModifyEvent(:final contentChanged) => 'modificado $p conteudo=$contentChanged',
    FileSystemDeleteEvent() => 'apagado $p',
    FileSystemMoveEvent(:final destination) => 'movido $p -> ${destination == null ? null : nome(base, destination)}',
  };
}

/// Faz `acao`, espera o evento que satisfaz `fim` (ou 5 s) e mais um pouco
/// para os que chegam juntos; imprime o conjunto dos eventos do passo.
Future<void> passo(List<String> vistos, String titulo, void Function() acao, bool Function(String) fim) async {
  final inicio = vistos.length;
  acao();
  for (var i = 0; i < 500 && !vistos.skip(inicio).any(fim); i++) {
    await Future.delayed(const Duration(milliseconds: 10));
  }
  await Future.delayed(const Duration(milliseconds: 200));
  print('$titulo: ${(vistos.sublist(inicio).toSet().toList()..sort())}');
}

Future<void> main() async {
  if (!Platform.isWindows) {
    print('observação comparada só no Windows');
    return;
  }
  print('suportado: ${FileSystemEntity.isWatchSupported}');
  final base = Directory.systemTemp.createTempSync('df_obs');
  final vistos = <String>[];
  final sub = base.watch().listen((e) => vistos.add(descrever(base, e)));

  final a = File('${base.path}\\a.txt');
  await passo(vistos, 'criar', () => a.writeAsStringSync('um'), (s) => s.startsWith('criado a.txt'));
  await passo(vistos, 'modificar', () => a.writeAsStringSync('dois', mode: FileMode.append),
      (s) => s.startsWith('modificado a.txt'));
  await passo(vistos, 'renomear', () => a.renameSync('${base.path}\\b.txt'), (s) => s.startsWith('movido'));
  await passo(vistos, 'subdiretório', () => Directory('${base.path}\\d').createSync(), (s) => s.startsWith('criado d'));
  await passo(vistos, 'apagar', () => File('${base.path}\\b.txt').deleteSync(), (s) => s.startsWith('apagado b.txt'));
  await sub.cancel();

  // Só os eventos pedidos: um observador de criações não vê a remoção.
  final outro = Directory('${base.path}\\o')..createSync();
  final criacoes = <String>[];
  final sub2 = outro.watch(events: FileSystemEvent.create).listen((e) => criacoes.add(descrever(outro, e)));
  await passo(criacoes, 'filtro', () {
    File('${outro.path}\\c.txt').createSync();
    File('${outro.path}\\c.txt').deleteSync();
  }, (s) => s.startsWith('criado c.txt'));
  await sub2.cancel();

  // Recursivo: o que muda num subdiretório chega com o caminho relativo.
  final arvore = Directory('${base.path}\\r')..createSync();
  Directory('${arvore.path}\\s').createSync();
  final profundos = <String>[];
  final sub3 = arvore.watch(recursive: true).listen((e) => profundos.add(descrever(arvore, e)));
  await passo(profundos, 'recursivo', () => File('${arvore.path}\\s\\x.txt').writeAsStringSync('x'),
      (s) => s.startsWith('criado s\\x.txt'));
  await sub3.cancel();

  // Um arquivo no lugar do diretório: nenhum evento, nenhum erro.
  final f = File('${base.path}\\f.txt')..writeAsStringSync('x');
  final doArquivo = <String>[];
  final sub4 = f.watch().listen((e) => doArquivo.add('${e.type}'), onError: (e) => doArquivo.add('erro ${e.runtimeType}'));
  f.writeAsStringSync('y');
  await Future.delayed(const Duration(milliseconds: 300));
  print('arquivo: $doArquivo');
  await sub4.cancel();

  // Depois do cancelamento, nada mais chega.
  final depois = vistos.length;
  File('${base.path}\\z.txt').writeAsStringSync('z');
  await Future.delayed(const Duration(milliseconds: 200));
  print('depois do cancelamento: ${vistos.length - depois}');

  // Caminho inexistente: o erro chega pelo fluxo.
  final erro = Completer<Object>();
  Directory('${base.path}\\nao_existe').watch().listen((_) {}, onError: erro.complete);
  final e = await erro.future;
  print('erro: ${e.runtimeType} ${(e as FileSystemException).message}');

  base.deleteSync(recursive: true);
  print('fim');
}
