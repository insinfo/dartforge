// dart:io nativo: RawDatagramSocket.joinMulticast/leaveMulticast (N03). Entra num
// grupo IPv4, recebe o próprio datagrama pelo laço de multicast, sai, e os
// erros do sistema (sair de novo, grupo unicast) viram OSError como na VM.
import 'dart:async';
import 'dart:io';

Future<void> main() async {
  final grupo = InternetAddress('239.255.43.21');
  final s = await RawDatagramSocket.bind(InternetAddress.anyIPv4, 0);
  s.multicastLoopback = true;
  s.joinMulticast(grupo);
  print('entrou');
  final recebido = Completer<String>();
  s.listen((e) {
    if (e == RawSocketEvent.read) {
      final d = s.receive();
      if (d != null && !recebido.isCompleted) recebido.complete(String.fromCharCodes(d.data));
    }
  });
  s.send('ola grupo'.codeUnits, grupo, s.port);
  print(await recebido.future.timeout(const Duration(seconds: 5), onTimeout: () => 'nada'));
  s.leaveMulticast(grupo);
  print('saiu');
  try {
    s.leaveMulticast(grupo);
    print('saiu de novo');
  } on OSError catch (e) {
    print('erro ao sair de novo: ${e.errorCode != 0}');
  }
  try {
    s.joinMulticast(InternetAddress('10.0.0.1'));
    print('entrou em unicast');
  } on OSError catch (e) {
    print('erro unicast: ${e.errorCode != 0}');
  }
  s.close();
}
