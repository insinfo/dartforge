// dart:io: RawDatagramSocket (UDP), ProcessSignal.watch e ProcessInfo.
import 'dart:io';
import 'dart:convert';
import 'dart:async';
Future<void> main() async {
  final a = await RawDatagramSocket.bind(InternetAddress.loopbackIPv4, 0);
  final b = await RawDatagramSocket.bind(InternetAddress.loopbackIPv4, 0);
  final recebido = Completer<String>();
  b.listen((e) {
    if (e == RawSocketEvent.read) {
      final d = b.receive();
      if (d != null) recebido.complete('${utf8.decode(d.data)} de ${d.address.address}');
    }
  });
  a.send(utf8.encode('udp!'), InternetAddress.loopbackIPv4, b.port);
  print(await recebido.future);
  a.close();
  b.close();
  final sub = ProcessSignal.sigusr1.watch().listen((s) => print('sinal $s'));
  Process.killPid(pid, ProcessSignal.sigusr1);
  await Future.delayed(Duration(milliseconds: 200));
  await sub.cancel();
  print(ProcessInfo.currentRss > 0);
}
