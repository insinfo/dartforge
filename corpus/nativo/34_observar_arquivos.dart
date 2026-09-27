// N02: `FileSystemEntity.watch` (inotify no Linux, como a VM): criação,
// modificação, mudança de nome, remoção e a remoção do próprio diretório,
// que encerra o fluxo; a observação de um arquivo só; `isWatchSupported`.
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

/// Os eventos sem as repetições seguidas (o sistema pode juntar ou separar
/// as escritas de um mesmo arquivo).
List<String> sem_repeticoes(Iterable<String> xs) {
  final r = <String>[];
  for (final x in xs) {
    if (r.isEmpty || r.last != x) r.add(x);
  }
  return r;
}

/// Faz `acao`, espera o evento que satisfaz `fim` (ou 5 s) e mais um pouco
/// para os que chegam juntos.
Future<void> passo(List<String> vistos, String titulo, void Function() acao, bool Function(String) fim) async {
  final inicio = vistos.length;
  acao();
  for (var i = 0; i < 500 && !vistos.skip(inicio).any(fim); i++) {
    await Future.delayed(const Duration(milliseconds: 10));
  }
  await Future.delayed(const Duration(milliseconds: 100));
  print('$titulo: ${sem_repeticoes(vistos.sublist(inicio))}');
}

Future<void> main() async {
  if (!Platform.isLinux) {
    print('observação comparada só no Linux');
    return;
  }
  print('suportado: ${FileSystemEntity.isWatchSupported}');
  final base = Directory.systemTemp.createTempSync('df_obs');
  final vistos = <String>[];
  final fim = Completer<void>();
  final sub = base.watch().listen((e) => vistos.add(descrever(base, e)), onDone: fim.complete);

  final a = File('${base.path}/a.txt');
  await passo(vistos, 'criar', () => a.writeAsStringSync('um'), (s) => s.startsWith('modificado a.txt'));
  await passo(vistos, 'modificar', () => a.writeAsStringSync('dois', mode: FileMode.append),
      (s) => s.startsWith('modificado a.txt'));
  await passo(vistos, 'renomear', () => a.renameSync('${base.path}/b.txt'), (s) => s.startsWith('movido'));
  await passo(vistos, 'subdiretório', () => Directory('${base.path}/d').createSync(), (s) => s.startsWith('criado d'));
  await passo(vistos, 'apagar', () => File('${base.path}/b.txt').deleteSync(), (s) => s.startsWith('apagado b.txt'));

  // Só os eventos pedidos: um observador de criações não vê a remoção (num
  // diretório à parte: dois observadores do mesmo caminho dividem a máscara
  // do inotify).
  final outro = Directory('${base.path}/o')..createSync();
  await Future.delayed(const Duration(milliseconds: 100));
  vistos.clear();
  final criacoes = <String>[];
  final sub2 = outro.watch(events: FileSystemEvent.create).listen((e) => criacoes.add(descrever(outro, e)));
  await passo(criacoes, 'filtro', () {
    File('${outro.path}/c.txt').createSync();
    File('${outro.path}/c.txt').deleteSync();
  }, (s) => s.startsWith('criado c.txt'));
  await sub2.cancel();
  outro.deleteSync();

  // Um arquivo só.
  final f = File('${base.path}/f.txt')..writeAsStringSync('x');
  final doArquivo = <String>[];
  final sub3 = f.watch().listen((e) => doArquivo.add('${e.type} ${e.path == f.path}'));
  await passo(doArquivo, 'arquivo', () => f.writeAsStringSync('y'), (s) => true);
  await sub3.cancel();
  f.deleteSync();
  await Future.delayed(const Duration(milliseconds: 50));

  // Apagar o próprio diretório encerra o fluxo.
  await Future.delayed(const Duration(milliseconds: 100));
  vistos.clear();
  Directory('${base.path}/d').deleteSync();
  base.deleteSync();
  await fim.future.timeout(const Duration(seconds: 5), onTimeout: () => print('fluxo não terminou'));
  print('fim: ${sem_repeticoes(vistos.where((s) => !s.contains(' o')))}');
  await sub.cancel();

  // Caminho inexistente: o erro chega pelo fluxo.
  final erro = Completer<Object>();
  Directory('${base.path}/nao_existe').watch().listen((_) {}, onError: erro.complete);
  final e = await erro.future;
  print('erro: ${e.runtimeType} ${(e as FileSystemException).message}');
}
