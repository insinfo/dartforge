// N04: `SocketControlMessage`/`ResourceHandle` sobre soquete de domínio Unix
// (`sendmsg`/`recvmsg` com `SCM_RIGHTS`): um arquivo e um soquete passados
// adiante e reabertos do outro lado.
import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

Future<void> main() async {
  // O Windows não tem domínio Unix no `dart:io` nem passagem de descritores
  // (na VM também não): o programa exercita os sistemas Unix.
  if (Platform.isWindows) {
    print('sem SCM_RIGHTS no Windows');
    return;
  }
  final dir = Directory.systemTemp.createTempSync('df_cm');
  final caminho = '${dir.path}/sock';
  final arq = File('${dir.path}/dados.txt')..writeAsStringSync('conteudo do arquivo');
  final servidor = await RawServerSocket.bind(InternetAddress(caminho, type: InternetAddressType.unix), 0);
  final aceito = Completer<RawSocket>();
  final extras = <RawSocket>[];
  servidor.listen((s) => aceito.isCompleted ? extras.add(s) : aceito.complete(s));
  final cliente = await RawSocket.connect(InternetAddress(caminho, type: InternetAddressType.unix), 0);
  final lado = await aceito.future;

  final aberto = arq.openSync();
  final msg = SocketControlMessage.fromHandles([ResourceHandle.fromFile(aberto)]);
  print('nível/tipo: ${msg.level != 0} ${msg.type != 0} ${msg.data.length}');
  final n = cliente.sendMessage([msg], Uint8List.fromList('ola'.codeUnits));
  print('enviados: $n');

  final recebido = Completer<SocketMessage>();
  lado.listen((e) {
    if (e == RawSocketEvent.read && !recebido.isCompleted) {
      final m = lado.readMessage();
      if (m != null) recebido.complete(m);
    }
  });
  final m = await recebido.future;
  print('dados: ${String.fromCharCodes(m.data)}');
  print('mensagens: ${m.controlMessages.length}');
  final handles = m.controlMessages.first.extractHandles();
  print('handles: ${handles.length}');
  final f = handles.first.toFile();
  print('lido: ${String.fromCharCodes(f.readSync(64))}');
  try {
    handles.first.toFile();
  } on StateError catch (e) {
    print('segunda vez: ${e.message}');
  }
  f.closeSync();
  aberto.closeSync();

  // Sem mensagens de controle.
  cliente.sendMessage([], Uint8List.fromList('so dados'.codeUnits));
  final m2 = await (() async {
    for (var i = 0; i < 200; i++) {
      final x = lado.readMessage();
      if (x != null) return x;
      await Future.delayed(const Duration(milliseconds: 5));
    }
    return null;
  })();
  print('m2: ${String.fromCharCodes(m2!.data)} ${m2.controlMessages.length}');

  // Um soquete de domínio Unix passado adiante e reaberto como `RawSocket`
  // (`toSocket`/`toRawDatagramSocket` a VM não suporta).
  final outro = await RawSocket.connect(InternetAddress(caminho, type: InternetAddressType.unix), 0);
  cliente.sendMessage([SocketControlMessage.fromHandles([ResourceHandle.fromRawSocket(outro)])], Uint8List.fromList([9]));
  final m3 = await (() async {
    for (var i = 0; i < 200; i++) {
      final x = lado.readMessage();
      if (x != null) return x;
      await Future.delayed(const Duration(milliseconds: 5));
    }
    return null;
  })();
  final r2 = m3!.controlMessages.first.extractHandles().first.toRawSocket();
  print('raw: ${m3.data} ${r2.address.type}');
  print('escritos: ${r2.write([1, 2, 3])}');
  while (extras.isEmpty) {
    await Future.delayed(const Duration(milliseconds: 5));
  }
  List<int>? lidos;
  for (var i = 0; i < 200 && lidos == null; i++) {
    lidos = extras.first.read();
    if (lidos == null) await Future.delayed(const Duration(milliseconds: 5));
  }
  print('do outro lado: $lidos');
  await r2.close();
  await outro.close();

  // Uma mensagem de controle que não leva descritores.
  print('sem recursos: ${SocketControlMessage.fromHandles([]).data.length}');

  await cliente.close();
  await lado.close();
  for (final s in extras) {
    await s.close();
  }
  await servidor.close();
  dir.deleteSync(recursive: true);
}
